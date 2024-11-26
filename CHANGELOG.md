# Serial Monitor changelog

All notable changes to the `Serial Monitor` crate will be documented in this file.

# Unreleased 0.3.0

### Added:

* removed the custom implementation of `Print` and `ScrollArea` and implemented the `log` crate and `egui_logger`
* Up to 4 Sentences highlightings using regex (thanks [@simon0356](https://github.com/simon0356))
* Groups settings in the side bar by category into collapsing menu. (thanks [@simon0356](https://github.com/simon0356))

## 0.3.0 - 14.10.2024 - Automatic Reconnection

### Added:

* Color-picker for curves
* Automatically reconnect when device becomes available again (only after unplugging)
* minor bug fixes

## 0.2.0 - 09.03.2024 - New Design, Improved Performance

### Added:

* [egui-phosphor](https://github.com/amPerl/egui-phosphor) icons for certain buttons
* multiple plots support (thanks [@oeb25](https://github.com/oeb25))
* implemented keyboard shortcuts
* improved serial transfer speed (thanks [@L-Trump](https://github.com/L-Trump))
* Bug fixes (thanks [@zimward](https://github.com/zimward))

## Earlier:

* code clean up (thanks [@lonesometraveler](https://github.com/lonesometraveler))
