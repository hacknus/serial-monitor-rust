# Serial Monitor

<a href="https://github.com/hacknus/serial-monitor-rust/releases"><img src="icons/icon.png" alt=“” width="100" height="100"> </img> </a>

A cross-platform serial monitor and plotter written entirely in rust, the GUI is written
using [egui](https://github.com/emilk/egui).  
Inspired by the serial monitor/plotter from the Arduino IDE, but both plotting and reading the traffic can be done
simultaneously.

## Installation:

### Download pre-built executables

[Binary bundles](https://github.com/hacknus/serial-monitor-rust/releases) are available for Linux, macOS and Windows.

Running the apple silicon binary (serial-monitor-aarch64-apple-darwin.app) may result to the message "Serial Monitor is
damaged and cannot be opened.", to get
around this you first need to run:  
`xattr -rd com.apple.quarantine Serial\ Monitor.app`

On Linux first install the following:

```sh
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

### Compile from source

The source code can be run using `cargo run` or bundled to a platform-executable using cargo bundle.  
Currently [cargo bundle](https://github.com/burtonageo/cargo-bundle) only supports linux and macOS
bundles [see github issue](https://github.com/burtonageo/cargo-bundle/issues/77).
As a work-around we can use [cargo wix](https://github.com/volks73/cargo-wix) to create a windows installer.

#### Debian & Ubuntu

```sh
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libudev-dev
cargo install cargo-bundle
cargo bundle
```

### Fedora Rawhide

```sh
dnf install clang clang-devel clang-tools-extra libxkbcommon-devel pkg-config openssl-devel libxcb-devel gtk3-devel atk fontconfig-devel libusbx-devel
```

#### macOS

```sh
cargo install cargo-bundle
cargo bundle
```

#### iOS

But this is probably not useful since iOS devices do not let you access serial devices because of sandboxing.

```sh
sudo xcodebuild -license
rustup target add aarch64-apple-ios
cargo install cargo-bundle
./ios-cargo ipa --ipad --release
```

#### Windows

```sh
cargo install cargo-wix
cargo wix
```

## Commandline Arguments
You can start the app with commandline arguments to automatically select a serial port and its settings:

```text
Usage: serial-monitor-rust [OPTIONS]

Positional arguments:
  device                   Serial port device to open on startup

Optional arguments:
  -b, --baudrate BAUDRATE  Baudrate (default=9600)
  -d, --databits DATABITS  Data bits (5, 6, 7, default=8)
  -f, --flow FLOW          Flow conrol (hard, soft, default=none)
  -s, --stopbits STOPBITS  Stop bits (default=1, 2)
  -p, --parity PARITY      Parity (odd, even, default=none)
  -F, --file FILE          Load data from a file instead of a serial port
  --column COLUMN-LABELS   Column labels, can be specified multiple times for more columns
  --color COLUMN-COLORS    Column colors (hex color without #), can be specified multiple times for more columns
  -h, --help
```

Example usage:

```sh
serial-monitor-rust /dev/ttyACM0 --baudrate 115200
```

You can also preconfigure the column settings.  The following example configures the name and color for two columns in the incoming data:

```sh
serial-monitor-rust --column Raw --color '808080' --column Temperature --color 'ff8000' /dev/ttyACM0
```

## Features:

- [X] Plotting and printing of data simultaneously
- [X] Smart data parser, works with ", " or "," or ":" or ": "
- [X] History of the past sent commands
- [X] Low CPU Usage, lightweight
- [X] Clear history options
- [X] Data Window width (number of displayed datapoints in plot) is adjustable
- [X] Cross-platform, fully written in Rust
- [X] Ability to save text to file
- [X] Ability to save the plot
- [X] Allow to put in labels for the different data columns (instead of column 1, 2, ...)
- [X] Allow to choose Data-bits, Flow-Control, Parity and Stop-Bits for Serial Connection
- [X] Saves the configuration for the serial port after closing and reloads them automatically upon selection
- [X] Option to save raw data to file
- [X] Use keyboard shortcuts (ctrl-S to save data, ctrl-shift-S to save plot, ctrl-X to clear plot)
- [X] Automatic reconnect after device has been unplugged
- [X] Color-picker for curves
- [X] Open a CSV file and display data in plot
- [ ] Allow to select (and copy) more than just the displayed raw traffic (also implement ctrl + A)
- [ ] Smarter data parser
- [ ] make serial print selectable and show corresponding datapoint in plot
- [ ] COM-Port names on Windows (display manufacturer, name, pid or vid of device?)
- [ ] current command entered is lost when navigating through the history
- [ ] command history is currently unlimited (needs an upper limit to prevent huge memory usage)
- [ ] data history is currently unlimited (needs an upper limit to prevent huge memory usage)
- [ ] ...

![Screenshot of the application on macOS](screenshot.png)

Tested on:

- macOS 12 Monterey x86
- macOS 13 Ventura x86
- macOS 13 Ventura ARM
- macOS 14 Sonoma ARM
- Debian 12 (Testing) x86
- Windows 10 x86
- ...

One might have to delete the `Cargo.lock` file before compiling.
