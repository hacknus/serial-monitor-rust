# Serial Monitor

<a href="https://github.com/hacknus/serial-monitor-rust/releases"><img src="icons/icon.png" alt= “” width="100" height="100"> </img> 

A cross-platform serial monitor and plotter written entirely in rust, the GUI is written
using [egui](https://github.com/emilk/egui).  
Inspired by the serial monitor/plotter from the Arduino IDE, but both plotting and reading the traffic can be done
simultaneously.  
Additionally, the output of the plot and the traffic can be saved to a file (TBD). The window of the plot can also be
adjusted.  
Data points in the received string between ", " or "," or ":" or ": " that can be parsed into floats will be plotted as
lines, everything else will be discarded without raising an error.

Features:

- [X] Plotting and printing of data simultaneously
- [X] Smart data parser, works with ", " or "," or ":" or ": "
- [X] History of the past sent commands
- [X] Low CPU Usage, lightweight
- [X] Clear history options
- [X] Data Window width is adjustable
- [X] Cross-platform, fully written in Rust
- [X] Ability to save text to file
- [X] Ability to save the plot
- [X] Allow to put in labels for the different data columns (instead of column 1, 2, ...)
- [X] Allow to choose Data-bits, Flow-Control, Parity and Stop-Bits for Serial Connection
- [X] Saves the configuration for the serial port after closing and reloads them automatically upon selection
- [ ] Save raw data to file (at least as an option)
- [ ] Smarter data parser
- [ ] make serial print selectable and show corresponding datapoint in plot
- [ ] COM-Port names on Windows (display manufacturer, name, pid or vid of device?)
- [ ] make side panel and plot/serial prompt be resizeable (snappy?)
- [ ] current command entered is lost when navigating through the history
- [ ] command history is currently unlimited (needs an upper limit to prevent huge memory usage)
- [ ] ...

![Screenshot of the application on macOS](screenshot.png)

The source code can be run using ```cargo run``` or bundled to a platform-executable using ```cargo bundle```.  
Currently [cargo bundle](https://github.com/burtonageo/cargo-bundle) only supports linux and macOS
bundles [see github issue](https://github.com/burtonageo/cargo-bundle/issues/77).
As a work-around we can use [cargo wix](https://github.com/volks73/cargo-wix) to create a windows installer.  
It can be compiled and run on all platforms.
Tested on:

- macOS 12.4 Monterey x86
- macOS 13.2.1 Ventura ARM
- Windows 10 x86
- ...

On Debian 12 (Testing) the following error occurred:

```
Error: glib-2.0 was not found in the pkg-config search path.
```

solved through

```
sudo apt-get install libgtk-3-dev
```

One might have to delete the ```Cargo.lock``` file before compiling.  
