#!/usr/bin/env python3
"""Drop a file on an X11 window, as a file manager does, over XDND.

    python3 scripts/smoke/xdnd_drop.py FILE WINDOW_ID [--x X --y Y]

WINDOW_ID is the target's X window id (decimal or 0x hex, e.g. from
`xdotool search --onlyvisible --name Separatrix | tail -1`; searching for the
full title fails outside a UTF-8 locale); the drop lands on its center unless
--x/--y give a point in root coordinates. The script plays the drag source:
it owns XdndSelection, sends XdndEnter, XdndPosition and XdndDrop, answers
the target's text/uri-list SelectionRequest with the file's URI, and waits
for XdndFinished. It exits 0 once the target accepted the drop and finished,
1 otherwise. Needs python-xlib and $DISPLAY.
"""

import argparse
import pathlib
import sys
import time

from Xlib import X, Xatom, display
from Xlib.protocol import event

XDND_VERSION = 5
TIMEOUT_S = 10.0


def parse_args():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("file", type=pathlib.Path)
    p.add_argument("window", type=lambda s: int(s, 0))
    p.add_argument("--x", type=int)
    p.add_argument("--y", type=int)
    return p.parse_args()


def xdnd_target(d, wid):
    """The window at or under `wid` that announces XdndAware, and its version."""
    aware = d.intern_atom("XdndAware")
    queue = [d.create_resource_object("window", wid)]
    while queue:
        w = queue.pop(0)
        prop = w.get_full_property(aware, Xatom.ATOM)
        if prop is not None and prop.value:
            return w, int(prop.value[0])
        queue.extend(w.query_tree().children)
    return None, 0


def server_time(d, w):
    """A real X timestamp: GTK refuses selection transfers stamped CurrentTime on some paths."""
    w.change_property(d.intern_atom("SEPX_XDND_TIME"), Xatom.STRING, 8, b"")
    d.flush()
    while True:
        e = d.next_event()
        if e.type == X.PropertyNotify and e.window.id == w.id:
            return e.time


def send(d, target, source, name, data):
    msg = event.ClientMessage(
        window=target,
        client_type=d.intern_atom(name),
        data=(32, [source.id] + data + [0] * (4 - len(data))),
    )
    target.send_event(msg, event_mask=0)
    d.flush()


def main():
    args = parse_args()
    path = args.file.resolve()
    if not path.is_file():
        sys.exit(f"xdnd_drop: no such file: {path}")
    uri = (path.as_uri() + "\r\n").encode()

    d = display.Display()
    root = d.screen().root
    target, version = xdnd_target(d, args.window)
    if target is None:
        sys.exit(f"xdnd_drop: no XdndAware window at or under {args.window:#x}")
    if args.x is None or args.y is None:
        g = target.get_geometry()
        origin = target.translate_coords(root, 0, 0)
        args.x = -origin.x + g.width // 2
        args.y = -origin.y + g.height // 2

    source = root.create_window(
        -10, -10, 1, 1, 0, X.CopyFromParent, event_mask=X.PropertyChangeMask
    )
    selection = d.intern_atom("XdndSelection")
    uri_list = d.intern_atom("text/uri-list")
    copy = d.intern_atom("XdndActionCopy")
    now = server_time(d, source)
    source.set_selection_owner(selection, now)
    d.flush()

    version = min(version, XDND_VERSION)
    send(d, target, source, "XdndEnter", [version << 24, uri_list])
    send(d, target, source, "XdndPosition", [0, (args.x << 16) | args.y, now, copy])

    accepted = dropped = finished = served = False
    deadline = time.monotonic() + TIMEOUT_S
    while time.monotonic() < deadline and not finished:
        if not d.pending_events():
            time.sleep(0.01)
            continue
        e = d.next_event()
        if e.type == X.ClientMessage:
            kind = d.get_atom_name(e.client_type)
            fmt, data = e.data
            if kind == "XdndStatus" and not dropped:
                accepted = bool(data[1] & 1)
                if not accepted:
                    break
                send(d, target, source, "XdndDrop", [0, now])
                dropped = True
            elif kind == "XdndFinished":
                finished = True
                if version >= 5:
                    accepted = bool(data[1] & 1)
        elif e.type == X.SelectionRequest and e.selection == selection:
            ok = e.target == uri_list
            prop = e.property if e.property != X.NONE else e.target
            if ok:
                e.requestor.change_property(prop, uri_list, 8, uri)
            reply = event.SelectionNotify(
                time=e.time,
                requestor=e.requestor,
                selection=e.selection,
                target=e.target,
                property=prop if ok else X.NONE,
            )
            e.requestor.send_event(reply, event_mask=0)
            d.flush()
            served = served or ok

    if not dropped:
        send(d, target, source, "XdndLeave", [0])
    source.destroy()
    d.flush()
    print(
        f"xdnd_drop: {path} on {target.id:#x} at ({args.x}, {args.y}): "
        f"accepted={accepted} served={served} finished={finished}"
    )
    return 0 if accepted and served and finished else 1


if __name__ == "__main__":
    sys.exit(main())
