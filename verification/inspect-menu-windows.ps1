$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class MenuWindowReadOnly {
 public delegate bool Proc(IntPtr h,IntPtr l);
 public struct RECT { public int L,T,R,B; }
 [DllImport("user32.dll")] public static extern bool EnumWindows(Proc p,IntPtr l);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out RECT r);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h,int i);
 public static void Dump(uint wanted) {
  EnumWindows((h,l)=> { uint p; GetWindowThreadProcessId(h,out p); if(p!=wanted)return true;
   var title=new StringBuilder(200);var cls=new StringBuilder(100); RECT r;GetWindowText(h,title,200);GetClassName(h,cls,100);GetWindowRect(h,out r);
   Console.WriteLine("pid={0} hwnd={1} visible={2} title={3} class={4} rect={5},{6},{7},{8} style={9:X8} exstyle={10:X8}",p,h,IsWindowVisible(h),title,cls,r.L,r.T,r.R,r.B,GetWindowLong(h,-16),GetWindowLong(h,-20)); return true;
  },IntPtr.Zero);
 }
}
'@
Get-Process sayall-windows-app -ErrorAction SilentlyContinue | ForEach-Object { [MenuWindowReadOnly]::Dump([uint32]$_.Id) }
Get-CimInstance Win32_Process -Filter "name='sayall-windows-app.exe'" | ForEach-Object { "pid=$($_.ProcessId) parent=$($_.ParentProcessId) started=$($_.CreationDate) keyhost=$($_.CommandLine -match '--key-host')" }
