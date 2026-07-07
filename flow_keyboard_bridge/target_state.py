from __future__ import annotations

from dataclasses import dataclass


@dataclass
class TargetState:
    remote_enabled: bool = False

    def enable_remote(self) -> None:
        self.remote_enabled = True

    def enable_local(self) -> None:
        self.remote_enabled = False

