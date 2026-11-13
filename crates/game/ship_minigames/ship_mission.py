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
TASK_BY_ID = {task_id: (label, opener) for task_id, label, opener in TASKS}

# These are the interaction points used by ECHO-7's interior.  The gameplay
# layer can pass one of these stable ids instead of knowing anything about
# Tkinter or the implementation of a minigame.
INTERACTION_TASKS = {
    "electrical_panel": "wiring",
    "life_support_panel": "filter",
    "ship_status_console": "code",
    "piloting_station": "asteroids",
}


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
        if task_id not in TASK_BY_ID:
            raise ValueError(f"unknown ship mission task: {task_id}")
        self.completed.add(task_id)
        return self.is_complete


class ShipInteractionMission:
    """Attach one minigame to each interactable ECHO-7 ship location."""

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
        self.active_task_id = None
        self.active_task = None

    def activate(self, interaction_id: str):
        """Open the task assigned to an interaction point.

        Returns the Tk task instance, or ``None`` when the point is already
        complete, unknown, or another minigame is currently open.
        """
        task_id = INTERACTION_TASKS.get(interaction_id)
        if task_id is None or task_id in self.progress.completed:
            return None
        if self.active_task is not None:
            return None

        _label, opener = TASK_BY_ID[task_id]
        self.active_task_id = task_id
        self.active_task = opener(
            self.parent,
            on_success=lambda task_id=task_id: self._complete(task_id),
        )
        return self.active_task

    def _complete(self, task_id: str):
        if task_id != self.active_task_id:
            return
        window = self.active_task.master if self.active_task is not None else None
        if window is not None and window.winfo_exists():
            window.destroy()
        self.active_task = None
        self.active_task_id = None
        mission_complete = self.progress.mark_completed(task_id)
        if self.on_progress:
            self.on_progress(self.progress)
        if mission_complete and self.on_complete:
            self.on_complete()

    def cancel(self):
        if self.active_task is not None:
            window = self.active_task.master
            if window.winfo_exists():
                window.destroy()
        self.active_task = None
        self.active_task_id = None


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
    import argparse
    import json

    root = tk.Tk()
    root.withdraw()

    parser = argparse.ArgumentParser()
    parser.add_argument("--task", choices=tuple(TASK_BY_ID))
    parser.add_argument("--event-file")
    args = parser.parse_args()

    if args.task:
        _label, opener = TASK_BY_ID[args.task]

        def task_complete():
            if args.event_file:
                with open(args.event_file, "a", encoding="utf-8") as stream:
                    stream.write(json.dumps({"task": args.task}) + "\n")
            root.destroy()

        task = opener(root, on_success=task_complete)
        # The game launches this process from a running renderer. Explicitly
        # raise and focus the Tk window so it cannot remain behind the game.
        # Mostrar la ventana del minijuego sin depender
        # de atributos internos de cada clase.
        for window in root.winfo_children():
            if isinstance(window, tk.Toplevel):
                window.deiconify()
                window.lift()
                window.focus_force()
    else:
        open_ship_maintenance_mission(
            root,
            on_complete=lambda: print("MISION DE MANTENIMIENTO COMPLETADA"),
            on_progress=lambda progress: print(
                f"PROGRESO {progress.count}/{progress.total}"
            ),
        )
    root.mainloop()
