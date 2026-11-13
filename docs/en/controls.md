# Controls

**English** | [Русский](../ru/controls.md)

The keyboard defaults are the original's. Keys and gamepad controls can be changed in the game's own options
(Options → Controls), where "Default" on a device restores its layout. A gamepad works only through these
bindings. On a standard pad (Xbox, PlayStation, Switch Pro) a profile whose controls were never changed gets
the pad's layout by itself when the pad is connected; otherwise pick the pad in the device list and press
"Default". The pad's layout is a modern one — left stick steers, RT throttle and LT brake (both analogue), A
handbrake, B look back, X reset the car, Y camera, RB / LB shift up / down, Start pause, L3 horn, d-pad up
lights, d-pad left / right turn signals, d-pad down hazards, right stick looks left / right. Wheels and other
devices get the original's layout. In the menus the d-pad or left stick moves, A selects. The keyboard keeps
working beside a pad: an action bound to the pad still answers its default key, unless that key is bound to
another action.

On a standard pad the race's dialogs and the replay bar have their own keys, which act only while a dialog or
the bar is up (never while driving): d-pad / left stick picks, A answers, B goes back.

| Screen | Pad |
|---|---|
| pause menu and its quit question | d-pad picks, A answers, B continues (on the question: "no") |
| result pages, notices | A closes the page, as Space does; on a failed mission's summary Y is "race again" |
| replay save | A saves under the typed name, or under the first free "Replay1", "Replay2"…; B closes it |
| "overwrite?" question | d-pad picks, A answers, B is "no" |
| replay playback | A play / pause, d-pad right / left speed up / down, d-pad up the result pages again, d-pad down restart, X save the replay, B quit, Y camera |

## Browser and touch

The game runs in the browser canvas. The fullscreen button in the browser toolbar enters browser
fullscreen; click the canvas first if the keyboard does not respond. Gamepads are read through the browser's
Gamepad API and work after the first button press or stick movement. Wheel force feedback is unavailable in
the browser, and controller rumble works in Chrome, Edge and Safari 16.4+; Firefox does not expose gamepad
rumble yet. On a pad the engine does not buzz continuously: rough shoulders, verges and grass rumble, tyre
scrub and wheelspin are felt near the grip limit, locked front wheels pulse while braking, and a gear change
gives a short kick. `FF.RoadGripVolume` sets the strength of the tyre and ABS cues; `FF.EngineVolume` only affects a
wheel, since a pad gets the engine through its sound. **Exit** in the game's front end saves the progress, leaves browser fullscreen and returns
to the site's home page; a page cannot close its own tab. Quitting a race from its pause menu returns to
the game's front end.

On touch devices an on-screen panel provides the essential controls: left / right steering, throttle, brake,
handbrake, pause, camera and gear up / down. The panel is shown for coarse pointers and can be used together
with a connected gamepad.

On touch devices, large steering and pedal buttons support sliding between buttons
without lifting the finger. Lower HUD instruments move above these buttons.
Menus use a D-pad, OK and Back; tapping the game itself still works. The ☰ button
opens the save, language and fullscreen toolbar. Tap an active in-game text field
to open the OS keyboard. Original character and length restrictions still apply.

With a mouse the toolbar stays open in the top right corner in menus. During a race the
game HUD fills every corner, so the toolbar folds into a ☰ at the bottom centre, in the
strip the dials and the track map leave free; its panel opens upwards above the button.

## Driving

| Key | Action |
|---|---|
| ← → | steer |
| ↑ | throttle |
| ↓ | brake / reverse |
| Space | handbrake |
| A / Z | shift up / down |
| R | put the car back on the track |
| H | horn |
| L | headlights |
| W / E | left / right turn signal |
| T | hazard lights |
| Q | abort the race |

## View and HUD

| Key | Action |
|---|---|
| C | next camera |
| B | look back |
| Y / I / U | look left / right / forward |
| F2 / F3 / F4 | rotate the camera clockwise / stop / counter-clockwise |
| F5 | HUD on / off |
| F6 | rear-view mirror on / off |
| F7 | map on / off |
| F8 | HUD transparency |
| 1 / 2 | cycle the HUD / the map |
| F9 / F10 | music / sound effects |
| P | pause; in the pause menu the arrows pick and Enter answers (continue, race again, quit, photo mode) |
| F1 | hide / show the pause menu or a result page, to look at the scene behind it |

At the end of a race **Space** closes the result pages one by one and leaves the last one.

## Photo mode

A free camera for a paused race or a replay. Enter it from the pause menu's **photo mode** item (on a pad:
d-pad to it, A), or with **K** in the pause menu or a replay. The race or the replay stands still while it is
up. It is not in the original game.

| Keyboard / mouse | Pad | Action |
|---|---|---|
| W A S D, Q / E | left stick, LT / RT | move, down / up |
| arrows, right mouse button | right stick | look |
| Z / X | LB / RB | roll |
| wheel, `-` / `=` | d-pad up / down | zoom |
| Shift / Ctrl | L3 | faster / slower |
| Enter, Space | A | save a picture; `6` picks its size (1x, 2x, 4x the window) |
| H | Y | the game's HUD |
| Tab, R | | the hint, reset the camera |
| K, Esc, P, Backspace | B | leave |

Pictures are saved as PNG downloads at the current canvas resolution. The browser may ask where to save the
file or put it in its downloads folder.

## Menus

Arrows move between rows and change values, **Enter** confirms, **Esc** goes back. The mouse works as in the
original.
