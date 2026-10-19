import { readFileSync } from 'node:fs'
import { runInNewContext } from 'node:vm'
import { describe, expect, it } from 'vitest'

const script = readFileSync('public/quick-menu.js', 'utf8')

describe('four-way menu mouse cancellation', () => {
  it('cancels the displayed generation once without running an option', () => {
    const document = window.document.implementation.createHTMLDocument()
    document.body.innerHTML = '<div id="up">未设置</div><button id="cancel-menu">取消</button>'
    const location = { search: '?generation=42', href: 'local-menu' }
    runInNewContext(script, { document, location, URLSearchParams })
    document.dispatchEvent(new Event('DOMContentLoaded'))
    const down = new MouseEvent('pointerdown', { bubbles: true, cancelable: true, button: 0 })
    document.getElementById('cancel-menu')!.dispatchEvent(down)
    expect(down.defaultPrevented).toBe(true)
    expect(location.href).toBe('https://sayall.invalid/menu-cancel?generation=42')
    location.href = 'already-submitted'
    document.getElementById('up')!.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0 }))
    expect(location.href).toBe('already-submitted')
  })

  it('does not cancel a preloaded page without a generation', () => {
    const document = window.document.implementation.createHTMLDocument()
    document.body.innerHTML = '<button id="cancel-menu">取消</button>'
    const location = { search: '', href: 'local-menu' }
    runInNewContext(script, { document, location, URLSearchParams })
    document.dispatchEvent(new Event('DOMContentLoaded'))
    document.getElementById('cancel-menu')!.click()
    expect(location.href).toBe('local-menu')
  })
})
