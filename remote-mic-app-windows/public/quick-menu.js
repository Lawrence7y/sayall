// Local menu mouse clicks only cancel; options remain remote-direction actions.
// The reserved URL is intercepted by the native navigation handler before any
// network request. It carries only a short-lived generation, never menu labels.
document.addEventListener('DOMContentLoaded', () => {
  const generation = new URLSearchParams(location.search).get('generation')
  if (!generation || !/^[1-9]\d*$/.test(generation)) return
  let submitted = false
  const cancel = event => {
    if (event.button !== 0 || submitted) return
    event.preventDefault()
    event.stopPropagation()
    submitted = true
    location.href = 'https://sayall.invalid/menu-cancel?generation=' + generation
  }
  // Cancel on DOWN before a WebView button's default focus/activation behavior.
  document.addEventListener('pointerdown', cancel)
  document.addEventListener('click', cancel)
})
