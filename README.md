# serial-monitor-rust
A cross-plattform serial monitor/plotter written entirely in rust, the GUI is written using [egui](https://github.com/emilk/egui).  
Inspired by the serial monitor/plotter from the Arduino IDE, but both plotting and reading the traffic can be done simultaneously.  
Additionally, the output of the plot and the traffic can be saved to a file (TBD). The window of the plot can also be adjusted.  
Data points in the received string between ", " that can be parsed into floats will be plotted as lines, everything else will be discarded without raising an error. More clever parsing/selection with other delimiters such as ": " will be added (TBD).

![Screenshot of the application on macOS](screenshot.png)
