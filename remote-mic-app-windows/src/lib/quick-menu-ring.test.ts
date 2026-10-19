import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const html = readFileSync('public/quick-menu.html', 'utf8')

describe('compact remote navigation ring', () => {
  it('uses only the cropped ring with four quiet function labels and a cancel center', () => {
    const doc = window.document.implementation.createHTMLDocument()
    doc.documentElement.innerHTML = html
    const ring = doc.querySelector('.ring')!
    expect(ring.querySelector('img')!.getAttribute('src')).toBe('remote-navigation-ring.png')
    for (const direction of ['up', 'down', 'left', 'right']) {
      expect(ring.querySelector(`#${direction}`)).not.toBeNull()
    }
    expect(ring.querySelector('#cancel-menu')!.getAttribute('aria-label')).toContain('取消')
    expect(doc.querySelector('.menu')).toBeNull()
    expect(html).toContain('background:transparent')
    expect(html).toContain('font-size:11px')
  })

  it('keeps the ring invisible until native show, then slides up with reduced-motion support', () => {
    expect(html).toContain('opacity:0')
    expect(html).toContain('.is-visible .ring')
    expect(html).toContain('@keyframes ring-enter')
    expect(html).toContain('translateY(22px)')
    expect(html).toContain('480ms')
    expect(html).toContain('prefers-reduced-motion:reduce')
  })

  it('centers labels on the cardinal ring sectors without crossing the center disk', () => {
    expect(html).toContain('transform:translate(-50%,-50%)')
    expect(html).toContain('.up{left:50%;top:12.5%;width:86px}')
    expect(html).toContain('.down{left:50%;top:87.5%;width:86px}')
    expect(html).toContain('.left{left:12.5%;top:50%;width:48px}')
    expect(html).toContain('.right{left:87.5%;top:50%;width:48px}')
  })

  it('ships a small RGBA ring crop rather than the whole remote', () => {
    const png = readFileSync('public/remote-navigation-ring.png')
    expect(png.readUInt32BE(16)).toBe(158)
    expect(png.readUInt32BE(20)).toBe(158)
    expect(png[25]).toBe(6) // PNG truecolor with alpha
  })
})
