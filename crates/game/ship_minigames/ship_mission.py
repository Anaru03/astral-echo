"""Mission coordinator for the four ECHO-7 ship minigames.

Each minigame can still be launched independently.  The coordinator is the
mission-facing entry point: it opens every task exactly once, keeps a shared
completion set, and reports success only after the complete set is finished.
"""

from dataclasses import dataclass, field
from typing import Callable, Optional

try:
    from .asteroids import open_asteroids_task
    from .code import open_code_task
    from .filter import open_o2_task
    from .wiring import open_wiring_task
except ImportError:  # direct: python ship_mission.py
    from asteroids import open_asteroids_task
    from code import open_code_task
    from filter import open_o2_task
    from wiring import open_wiring_task


TASKS = (
    ("wiring", "Restaurar la distribucion electrica", open_wiring_task),
    ("filter", "Restablecer el soporte vital", open_o2_task),
    ("code", "Validar los sistemas de acceso", open_code_task),
    ("asteroids", "Asegurar la defensa de la nave", open_asteroids_task),
)


@dataclass
class ShipMissionProgress:
    """Serializable-free mission state shared by the UI and callbacks."""

    completed: set[str] = field(default_factory=set)

    @property
    def total(self) -> int:
        return len(TASKS)

    @property
    def count(self) -> int:
        return len(self.completed)

    @property
    def is_complete(self) -> bool:
        return self.count == self.total

    def mark_completed(self, task_id: str) -> bool:
        """Mark a task once and return whether the whole mission is complete."""
        if task_id not in {task[0] for task in TASKS}:
            raise ValueError(f"unknown ship mission task: {task_id}")
        self.completed.add(task_id)
        return self.is_complete


class ShipMaintenanceMission:
    """Run all ship minigames as one ordered mission."""

    def __init__(
        self,
        parent,
        on_complete: Optional[Callable[[], None]] = None,
        on_progress: Optional[Callable[[ShipMissionProgress], None]] = None,
    ):
        self.parent = parent
        self.on_complete = on_complete
        self.on_progress = on_progress
        self.progress = ShipMissionProgress()
        self.index = 0
        self.current_task = None
        self.current_window = None
        self.finished = False

    def start(self):
        self._open_next()
        return self

    def _open_next(self):
        if self.finished or self.index >= len(TASKS):
            self._finish()
            return

        task_id, _label, opener = TASKS[self.index]
        self.current_task = opener(
            self.parent,
            on_success=lambda task_id=task_id: self._task_completed(task_id),
        )
        self.current_window = self.current_task.master

    def _task_completed(self, task_id: str):
        if self.finished:
            return
        if self.index >= len(TASKS) or task_id != TASKS[self.index][0]:
            return

        # A completed child must not remain open while the next minigame is
        # modal. This also prevents duplicate callbacks from advancing twice.
        if self.current_window is not None and self.current_window.winfo_exists():
            self.current_window.destroy()
        self.current_task = None
        self.current_window = None

        self.progress.mark_completed(task_id)
        self.index += 1
        if self.on_progress:
            self.on_progress(self.progress)

        if self.progress.is_complete:
            self._finish()
        else:
            self._open_next()

    def _finish(self):
        if self.finished:
            return
        self.finished = True
        if self.on_complete:
            self.on_complete()

    def cancel(self):
        if self.current_window is not None and self.current_window.winfo_exists():
            self.current_window.destroy()
        self.current_task = None
        self.current_window = None


def open_ship_maintenance_mission(parent, on_complete=None, on_progress=None):
    """Open the complete ECHO-7 maintenance mission."""
    return ShipMaintenanceMission(
        parent,
        on_complete=on_complete,
        on_progress=on_progress,
    ).start()


if __name__ == "__main__":
    import tkinter as tk

    root = tk.Tk()
    root.withdraw()
    open_ship_maintenance_mission(
        root,
        on_complete=lambda: print("MISION DE MANTENIMIENTO COMPLETADA"),
        on_progress=lambda progress: print(
            f"PROGRESO {progress.count}/{progress.total}"
        ),
    )
    root.mainloop()
