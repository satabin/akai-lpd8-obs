# Akai LPD8 for OBS

This small utility allows to easily configure an [Akai LPD8 Mk2](https://www.akaipro.com/lpd8-mk2.html) to control [OBS](https://obsproject.com/).

You can configure the 8 pads in PC and CC modes, as well as the 8 faders. Configuration is made through a [TOML](https://toml.io) file.

The program talks to OBS via its WebSocket API, so be sure to start the WebSocket server on OBS 28+.

**Note:** this program has only been tested on Linux so far. Other platform should work, but I don't own any of them. Contributions are welcome if you make it work on other platforms.

## Configuration

### Configurable inputs

Following controller inputs are configurable for the controller:

 - `pad1`
 - `pad2`
 - `pad3`
 - `pad4`
 - `pad5`
 - `pad6`
 - `pad7`
 - `pad8`
 - `fader1`
 - `fader2`
 - `fader3`
 - `fader4`
 - `fader5`
 - `fader6`
 - `fader7`
 - `fader8`

### Possible actions

#### `SetScene`

This action takes a parameter `name` indicating the scene name to transition to when the controller input it triggered.

Example:

```toml
<input>.action = "SetScene"
<input>.name = "Scene Name"
```

#### `SetVolume`

This action set the volume either to the control change data value (if set to `pass`) or to a fix value if set to a number between 0 and 100 (inclusive).

Volume is set in _multiplier_ mode (from 0 to 100%).

Fader values from controller are from 0 to 127 (inclusive), `0` representing _0%_ and `127` representing `100%`.

In PC mode, the pad value is always `0`.

Example:

```toml
<input>.action = "SetVolume"
<input>.value = "pass" # pass the current value of the controller input

<input>.action = "SetVolume"
<input>.value = 10 # always sets the volume to 10%
```

#### `ToggleInput`

This action takes a parameter `name` indicating the OBS input name to mute/unmute when the controller input is triggered.

Example:

```toml
<input>.action = "ToggleInput"
<input>.name = "Microphone"
```

#### `EnableSceneItem`

This action takes a parameter `name` indicating the OBS scene item (aka source) name to enable when the controller input is triggered.

Example:

```toml
<input>.action = "EnableSceneItem"
<input>.name = "Web Browser"
```

#### `DisableSceneItem`

This action takes a parameter `name` indicating the OBS scene item (aka source) name to disable when the controller input is triggered.

The configuration file allows following sections

Example:

```toml
<input>.action = "DisableSceneItem"
<input>.name = "Web Browser"
```

### `program_changes`

The `program_changes` section is a map from a pad when in PC mode to an action.

Example:

```toml
[program_changes]
# PAD 1, when pressed in PC mode will set the current scene to `Start`
pad1.action = "SetScene"
pad1.name = "Start"

# PAD 2, when pressed in PC mode will set the current scene to `In Game`
pad2.action = "SetScene"
pad2.name = "In Game"

# PAD 8, when pressed in PC mode will toggle the `Microphone` input
pad9.action = "ToggleInput"
pad9.name = "Microphone"
```

or (equivalently):

```toml
[program_changes]
pad1 = { action = "SetScene", name = "Start" }
pad2 = { action = "SetScene", name = "In Game" }
pad8 = { action = "ToggleInput", name = "Microphone" }
```

### `control_changes`

The `control_changes` section is a list of maps from a pad when in CC mode or a fader to a conditional action.

A conditional action is like a standard action but has an optional extra field `on` that can indicate on which value the action triggers. Valid values for the `on` field are integer between 0 and 127 (inclusive).

Example:

```toml
[[control_changes]]
# PAD1, when pressed in CC mode, will enable the source named "Web Browser"
pad1.action = EnableSceneItem
pad1.name = "Web Browser"

# PAD1, when released in CC mode, will disable the source named "Web Browser"
pad1.on = 0
pad1.action = DisableSceneItem
pad1.name = "Web Browser"

# Fader 1 (K1) when the value change will set the volume to the current fader value
fader1.action = "SetVolume"
fader1.value = "pass"
```
