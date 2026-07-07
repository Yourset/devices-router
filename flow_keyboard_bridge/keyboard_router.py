from __future__ import annotations

from dataclasses import dataclass, field


VK_CONTROL = 0x11
VK_MENU = 0x12
VK_ESCAPE = 0x1B
VK_1 = 0x31
VK_2 = 0x32


@dataclass(frozen=True)
class RawKeyEvent:
    action: str
    vk_code: int


@dataclass
class KeyboardRouter:
    remote_enabled: bool = False
    pressed: set[int] = field(default_factory=set)
    stop_requested: bool = False

    def handle(self, event: RawKeyEvent) -> bool:
        if event.action == "down":
            self.pressed.add(event.vk_code)
            if self._ctrl_alt_down() and event.vk_code == VK_1:
                self.remote_enabled = False
                return True
            if self._ctrl_alt_down() and event.vk_code == VK_2:
                self.remote_enabled = True
                return True
            if self._ctrl_alt_down() and event.vk_code == VK_ESCAPE:
                self.stop_requested = True
                return True
        elif event.action == "up":
            self.pressed.discard(event.vk_code)

        return self.remote_enabled

    def _ctrl_alt_down(self) -> bool:
        return VK_CONTROL in self.pressed and VK_MENU in self.pressed

