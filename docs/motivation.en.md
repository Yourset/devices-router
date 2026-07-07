# Motivation: The Honest Version

This tool did not start as an attempt to build a grand KVM system or to challenge Logitech Flow.

The real use case was much more ordinary:

```text
I wanted to vibe code on another PC as the development machine,
while keeping this main PC free for things like League of Legends,
media, and whatever else was already open.
Logitech Flow could move the mouse there, but my normal keyboard did not follow.
So I built the missing keyboard-following layer.
```

That is why Devices Router is intentionally narrow:

- It does not replace Logitech Flow.
- It does not reverse engineer Flow's private protocol.
- It does not emulate Logitech devices.
- It is not a full remote desktop.
- It only fills the annoying gap: make a normal keyboard follow the mouse to another PC.

This also explains the product priorities:

- Double-click to use, no command line every time.
- Remote finds the host automatically, no memorizing IP addresses.
- Mouse moves there, keyboard follows there.
- Remote updates from the host, no manual exe copying.
- Logs can be copied, exported, and cleared for quick debugging.

At its core, this is a small quality-of-life utility for using two PCs more comfortably. One machine can stay focused on development, the other can stay available for games or everyday windows, without constantly switching between two keyboards.

The fun part is that the project did not begin with a perfect architecture. It began with a very human lazy-workflow need, then grew through real testing: connection issues, input injection bugs, update failures, stale status, and all the small details that make a tool feel usable.
