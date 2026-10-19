use std::{process::{Command, Stdio}, os::windows::process::CommandExt, time::{Duration, Instant}, thread, io::Read};
#[derive(Debug)] struct Display { id: String, brightness: i32, kind: String }
fn parse(s: &str) -> Result<Vec<Display>, String> {
    let mut clean=String::new(); let mut it=s.chars();
    while let Some(c)=it.next() { if c=='\x1b' { if it.next()!=Some('[') {return Err("invalid escape".into())} for x in it.by_ref() {if x.is_ascii_alphabetic(){break}} } else {clean.push(c)} }
    let mut out=Vec::new();
    for block in clean.split("MonitorNum:").skip(1) {
        let field=|key:&str| block.lines().find_map(|l|l.trim().strip_prefix(key).map(str::trim)).ok_or(format!("missing {key}"));
        out.push(Display{id:field("MonitorID:")?.to_owned(),brightness:field("Brightness:")?.parse().map_err(|_|"invalid brightness")?,kind:field("Type:")?.to_owned()});
    }
    if out.is_empty(){return Err("empty list".into())} Ok(out)
}
fn cli(args:&[String])->Result<(String,u128),String>{
    let exe=std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("no localappdata")?).join("Programs/twinkle-tray/Twinkle Tray.exe");
    let start=Instant::now();
    let mut child=Command::new(exe).args(args).creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|_|"spawn failed")?;
    let mut stdout=child.stdout.take().unwrap();let mut stderr=child.stderr.take().unwrap();
    let o=thread::spawn(move||{let mut b=Vec::new();stdout.read_to_end(&mut b).map(|_|b)});
    let e=thread::spawn(move||{let mut b=Vec::new();stderr.read_to_end(&mut b).map(|_|b)});
    let status=loop {match child.try_wait(){Ok(Some(s))=>break Ok(s),Ok(None) if start.elapsed()<Duration::from_secs(8)=>thread::sleep(Duration::from_millis(10)),_=>{let _=child.kill();let _=child.wait();break Err("timeout or wait failure")}}};
    let bytes=o.join().map_err(|_|"stdout join")?.map_err(|_|"stdout read")?;
    let _=e.join(); if !status?.success(){return Err("nonzero exit".into())}
    Ok((String::from_utf8(bytes).map_err(|_|"invalid UTF8")?,start.elapsed().as_millis()))
}
#[repr(C)] struct Rect{i:i32,j:i32,k:i32,l:i32}
#[repr(C)] #[derive(Clone,Copy)] struct Physical{handle:isize,description:[u16;128]}
#[link(name="user32")] unsafe extern "system" {fn EnumDisplayMonitors(dc:isize,clip:*const Rect,callback:unsafe extern "system" fn(isize,isize,*mut Rect,isize)->i32,data:isize)->i32;}
#[link(name="dxva2")] unsafe extern "system" {
 fn GetNumberOfPhysicalMonitorsFromHMONITOR(m:isize,n:*mut u32)->i32;
 fn GetPhysicalMonitorsFromHMONITOR(m:isize,n:u32,p:*mut Physical)->i32;
 fn DestroyPhysicalMonitor(h:isize)->i32;
 fn GetVCPFeatureAndVCPFeatureReply(h:isize,c:u8,t:*mut u32,v:*mut u32,max:*mut u32)->i32;
 fn SetVCPFeature(h:isize,c:u8,v:u32)->i32;
}
struct Handles(Vec<Physical>); impl Drop for Handles{fn drop(&mut self){for p in &self.0{unsafe{DestroyPhysicalMonitor(p.handle);}}}}
unsafe extern "system" fn enumerate(m:isize,_:isize,_:*mut Rect,data:isize)->i32 {
 let out=unsafe{&mut *(data as *mut Vec<Physical>)};let mut n=0;
 if unsafe{GetNumberOfPhysicalMonitorsFromHMONITOR(m,&mut n)}!=0 {let mut v=vec![Physical{handle:0,description:[0;128]};n as usize];if unsafe{GetPhysicalMonitorsFromHMONITOR(m,n,v.as_mut_ptr())}!=0{out.extend(v)}} 1
}
fn read(h:isize)->Option<(u32,u32)>{let(mut t,mut v,mut max)=(0,0,0);if unsafe{GetVCPFeatureAndVCPFeatureReply(h,0x10,&mut t,&mut v,&mut max)}!=0{Some((v,max))}else{None}}
fn offset(d:&Display,n:i32)->Result<u128,String>{cli(&[format!("--MonitorID={}",d.id),format!("--Offset={n}")]).map(|x|x.1)}
fn observe(h:isize,base:u32,up:bool)->Result<(u32,u128),String>{let s=Instant::now();while s.elapsed()<Duration::from_secs(4){if let Some((v,_))=read(h){if (up&&v>base)||(!up&&v<base){return Ok((v,s.elapsed().as_millis()))}}thread::sleep(Duration::from_millis(100));}Err("no hardware brightness change within 4 seconds".into())}
fn run()->Result<(),String>{
 let (text,ms)=cli(&["--List".into()])?;let displays=parse(&text)?;
 println!("list result=passed count={} utf8=true ansi_stripped=true elapsed_ms={ms}",displays.len());
 for (i,d) in displays.iter().enumerate(){println!("display index={i} type={} brightness={}",d.kind,d.brightness)}
 let mut v=Vec::new();if unsafe{EnumDisplayMonitors(0,std::ptr::null(),enumerate,&mut v as *mut _ as isize)}==0{return Err("enumeration failed".into())}let handles=Handles(v);
 let hardware:Vec<_>=handles.0.iter().filter_map(|p|read(p.handle).map(|(v,m)|(p.handle,v,m))).collect();
 println!("hardware readable_count={}",hardware.len());for (_,v,m) in &hardware{println!("hardware brightness_raw={v} max={m}")}
 if !std::env::args().any(|a|a=="--exercise" || a=="--all-serial"){return Ok(())}
 let targets:Vec<_>=displays.iter().filter(|d|d.kind=="ddcci").collect();
 if targets.len()!=1||hardware.len()!=1{return Err("exercise requires exactly one CLI ddcci target and one readable hardware monitor".into())}
 let d=targets[0];let (h,base,max)=hardware[0];if base<5||base+5>=max||d.brightness<5||d.brightness>94{return Err("brightness near boundary; not exercising".into())}
 let result=(||{
   if std::env::args().any(|a|a=="--all-serial") {
     for round in 1..=3 {
       let mut times=Vec::new();
       for _ in 0..3 { times.push(cli(&["--All".into(),"--Offset=1".into(),"--Overlay".into()])?.1); }
       thread::sleep(Duration::from_secs(1));
       let after=read(h).ok_or("all serial read failed")?.0;
       println!("all_serial round={round} count=3 cli_ms={times:?} before={base} after={after} expected={}",base+3);
       if after!=base+3 { return Err("CLI-only all serial accumulation mismatch".into()) }
       cli(&["--All".into(),format!("--Set={}",d.brightness)])?;
       thread::sleep(Duration::from_secs(1));
       if read(h).map(|x|x.0)!=Some(base){return Err("all serial restoration mismatch".into())}
     }
     println!("all_serial result=passed requests=9 hardware_confirmed=true");
     return Ok(());
   }
   let ms=offset(d,1)?;let(v,wait)=observe(h,base,true)?;println!("single_up result=passed cli_ms={ms} readback_wait_ms={wait} before={base} after={v}");
   let ms=offset(d,-1)?;let(v,wait)=observe(h,v,false)?;println!("single_down result=passed cli_ms={ms} readback_wait_ms={wait} after={v}");
   if v!=base{return Err("baseline drift after up/down".into())}
   println!("idle phase=started seconds=60");thread::sleep(Duration::from_secs(60));
   if read(h).map(|x|x.0)!=Some(base){return Err("brightness drift during idle".into())}
   let start=Instant::now();let ms=offset(d,1)?;let(v,_)=observe(h,base,true)?;println!("idle_first result=passed cli_ms={ms} hardware_total_ms={} after={v}",start.elapsed().as_millis());
   cli(&[format!("--MonitorID={}",d.id),format!("--Set={}",d.brightness)])?;thread::sleep(Duration::from_secs(1));
   if read(h).map(|x|x.0)!=Some(base){return Err("baseline not restored before burst".into())}
   let start=Instant::now();let mut jobs=Vec::new();
   for _ in 0..3{let id=d.id.clone();jobs.push(thread::spawn(move||cli(&[format!("--MonitorID={id}"),"--Offset=1".into()]).map(|x|x.1)));thread::sleep(Duration::from_millis(100));}
   let mut times=Vec::new();for j in jobs{times.push(j.join().map_err(|_|"burst join")??)}
   thread::sleep(Duration::from_secs(1));let after=read(h).ok_or("burst read failure")?.0;
   println!("burst count=3 submit_interval_ms=100 cli_ms={times:?} total_ms={} before={base} after={after} expected_raw_if_linear={}",start.elapsed().as_millis(),base+3);
   println!("parallel_burst result={} reason=hardware_accumulation",if after==base+3{"passed"}else{"failed"});
   cli(&[format!("--MonitorID={}",d.id),format!("--Set={}",d.brightness)])?;thread::sleep(Duration::from_secs(1));
   if read(h).map(|x|x.0)!=Some(base){return Err("baseline not restored before serial test".into())}
   let mut current=base;let mut serial_times=Vec::new();let serial_start=Instant::now();
   for _ in 0..3 {let started=Instant::now();offset(d,1)?;let (next,_)=observe(h,current,true)?;if next!=current+1{return Err("serial hardware step mismatch".into())}current=next;serial_times.push(started.elapsed().as_millis());}
   println!("serial_burst result=passed count=3 hardware_confirmed=true step_ms={serial_times:?} total_ms={} before={base} after={current}",serial_start.elapsed().as_millis());Ok(())
 })();
 // Always restore after attempting a write, including failures; public CLI first, public DDC fallback.
 let restored_cli=cli(&[format!("--MonitorID={}",d.id),format!("--Set={}",d.brightness)]);thread::sleep(Duration::from_secs(1));
 let mut fallback=false;if read(h).map(|x|x.0)!=Some(base){fallback=true;unsafe{SetVCPFeature(h,0x10,base);}thread::sleep(Duration::from_secs(1));}
 let restored=read(h).map(|x|x.0)==Some(base);println!("restore hardware_confirmed={restored} cli_completed={} direct_ddc_fallback={fallback} initial_raw={base}",restored_cli.is_ok());
 if !restored{return Err("RESTORATION FAILED: manual brightness check required".into())}result
}
fn main(){if let Err(e)=run(){eprintln!("probe result=failed reason={e}");std::process::exit(1)}}
#[cfg(test)] mod tests{use super::*;#[test]fn parses_unicode_ansi_and_zero_number(){let s="\x1b[36mMonitorNum:\x1b[0m 0\nMonitorID: test\nName: 显示器 1\nBrightness: 50\nType: none\n\nMonitorNum: 1\nMonitorID: second\nName: S82\nBrightness: 11\nType: ddcci\n";let d=parse(s).unwrap();assert_eq!(d.len(),2);assert_eq!(d[1].brightness,11);assert_eq!(d[0].kind,"none")}#[test]fn rejects_empty_or_missing_fields(){assert!(parse("").is_err());assert!(parse("MonitorNum: 1\nBrightness: bad\n").is_err())}}
