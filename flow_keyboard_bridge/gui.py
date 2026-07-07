from __future__ import annotations

import os
import queue
import sys
import threading
import tkinter as tk
from tkinter.scrolledtext import ScrolledText


class QueueWriter:
    def __init__(self, output_queue: queue.Queue[str]) -> None:
        self.output_queue = output_queue

    def write(self, text: str) -> int:
        if text:
            self.output_queue.put(text)
        return len(text)

    def flush(self) -> None:
        return None


class BridgeWindow:
    def __init__(self, title: str, subtitle: str, worker) -> None:
        self.output_queue: queue.Queue[str] = queue.Queue()
        self.root = tk.Tk()
        self.root.title(title)
        self.root.geometry("720x420")
        self.root.protocol("WM_DELETE_WINDOW", self.close)

        self.status = tk.Label(
            self.root,
            text=subtitle,
            anchor="w",
            padx=12,
            pady=10,
            font=("Microsoft YaHei UI", 11),
        )
        self.status.pack(fill="x")

        self.log = ScrolledText(self.root, height=18, font=("Consolas", 10))
        self.log.pack(fill="both", expand=True, padx=12, pady=(0, 12))
        self.log.insert("end", "正在启动...\n")
        self.log.configure(state="disabled")

        sys.stdout = QueueWriter(self.output_queue)
        sys.stderr = QueueWriter(self.output_queue)

        self.thread = threading.Thread(target=worker, daemon=True)
        self.thread.start()
        self.root.after(100, self.flush_logs)

    def flush_logs(self) -> None:
        changed = False
        while True:
            try:
                text = self.output_queue.get_nowait()
            except queue.Empty:
                break
            self.log.configure(state="normal")
            self.log.insert("end", text)
            changed = True
        if changed:
            self.log.see("end")
            self.log.configure(state="disabled")
        self.root.after(100, self.flush_logs)

    def close(self) -> None:
        os._exit(0)

    def run(self) -> None:
        self.root.mainloop()
