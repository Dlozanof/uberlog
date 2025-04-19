# Uberlog

An opinionated-ish embedded development tool that strives to be the next step in the `picocom` (or alternative) -> `logfile` -> `cat/grep` workflow. People working in many other areas have [fancy](https://log-viewer.opcodes.io/) [things](https://www.logviewplus.com/), but firmware engineers seem to think they have no right to have nice things. Just log the output of the serial port into a file and `grep` onto it, maybe some bash script to add a bit of coloring. **Not on my watch.**

Be aware that `uberlog` does not intend to be a self contained all-around solution to your workflow, as it would be impossible to handle every specific framework. Thus it will not do anything that can be done with *nix tools, and only supports text-based logging via UART/RTT out of the box. If that is not the case of your project, then you need to use a different program to stream the logs of your device into a text file; `uberlog` can take that file as input and stream logs from it real-time.

# Features

- It can read logs from three different kinds of source:
    - UART
    - RTT
    - File (real time stream, sort of `tail -f`)
- As many simultaneous inputs as desired, so multi-MCU communication or server-MCU interaction can be easily understood.
- Several filtering functionalities:
    - Highlight
    - Exclude logs that contain a given expression
    - Include only logs that contain a given expression
- Reset the device
- Stream logs to a file
- \[soon\] Manage a supported power supply
- \[soon\] Flash firmware

# Usage

The tool depends on the creation of a `.gadget.yaml` file inside the project directory that contains the information regarding your [target](https://probe.rs/targets):

```
power: !Dp100
  voltage: 5
  current: 0.5
targets:
- name: Main app (RTT)
  processor: CC2650
  log_backend: !Rtt
    elf_path: /path/to/binary.elf
  probe_id: PROBE_1_SERIAL
- name: Secondary processor (UART)
  processor: STM32F7
  log_backend: !Uart
    dev: /dev/ttyACM0
    baud: 115200
  probe_id: PROBE_2_SERIAL
```

Then you launch the tool from inside your project folder and it will parse `.gadget.yaml` know how to interact with your devices.
 
The fields are self explanatory, but note that `name` is whatever you want to name the target in the UI, and `processor` comes from `probe-rs` list of targets [link](https://probe.rs/targets). This means of course that the MCU management (flashing/reset/RTT) side of the project is done by the incredible `probe-rs` [project](https://probe.rs/). Go star it if you did not do it yet.

When the project is a bit more mature I will improve in the documentation, since it is quite prone to change, but I will always keep (famous last words) an up to date example here so you can just copy/paste and adapt it. StackOverflow style :D.

# Installation

So far no package manager is supported, only build for source like a madman. But assuming you have [Rust](https://www.rust-lang.org/tools/install) in your computer you can just:

```
cargo install --path .
```

And have your binary available in `$HOME/.cargo/bin/uberlog`. From this point you can do whatever you want to make the app available. Typically adding `$HOME/.cargo/bin` to your `$PATH`. But if you are reading this chances are you have a different way to managing binaries.
