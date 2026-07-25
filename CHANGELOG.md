# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

## [1.30.17] - 2026-07-17

Only dependencies were updated.

## [1.30.16] - 2026-07-13

Only dependencies were updated.

## [1.30.11] - 2026-07-05

Only dependencies were updated.

## [1.30.10] - 2026-06-26

Only dependencies were updated.

## [1.30.9] - 2026-06-19

Only dependencies were updated.

## [1.30.8] - 2026-06-04

Only dependencies were updated.

## [1.30.7] - 2026-05-19

Only dependencies were updated.

## [1.30.6] - 2026-05-06

Only dependencies were updated.

## [1.30.5] - 2026-05-03

### Fixed

- fix: ignore meta service as type when browsing #357

## [1.30.4] - 2026-04-20

Only dependencies were updated.

## [1.30.3] - 2026-04-10

Only dependencies were updated.

## [1.30.2] - 2026-04-05

Only dependencies were updated.

## [1.30.1] - 2026-04-04

### Added

- feat: set mdns ip check interval to 1s #307
- feat: add interface info to addresses #309
- feat: limit services list to 5 items for larger details view #311

## [1.29.2] - 2026-03-19

### Added

- feat: add IP-based URLs for HTTP services in get_urls() #294

## [1.29.1] - 2026-03-16

### Added

- feat: add URL selection popup for services with multiple URLs #289

## [1.29.0] - 2026-03-16

### Added

- feat: add Enter key to open service URL in browser #287

## [1.28.1] - 2026-03-15

### Fixed

- fix: spawn browser detached task to avoid blocking event loop #286

## [1.28.0] - 2026-03-15

### Added

- feat: make service types panel width dynamic based on content #285

## [1.27.1] - 2026-03-12

### Fixed

- fix: render error messages above help popup #277

## [1.27.0] - 2026-03-12

Only dependencies were updated.

## [1.26.3] - 2026-03-11

Only dependencies were updated.

## [1.26.2] - 2026-03-11

### Changed

- refactor: simplify disabling interfaces when using specific ones #267

### Fixed

- fix: documentation block for run_tui #268

## [1.26.1] - 2026-03-07

### Added

- feat: add flapping filter keyword #255

### Changed

- refactor: extract user input handling to dedicated module #254

## [1.25.1] - 2026-03-04

### Fixed

- fix: calculate wrapped line count for popup scroll #244

## [1.25.0] - 2026-03-03

### Added

- feat: add ability to browse new service types at runtime #239

## [1.24.1] - 2026-03-02

### Fixed

- fix: update services flapping status after loading a state dump #231

## [1.24.0] - 2026-03-02

### Added

- feat: remove --no-debounce option and improve flap detection #230
- feat: regression test for non null interfaces round-trip #229

### Fixed

- fix: include CLI options in JSON state dump #228

## [1.23.6] - 2026-03-01

Only dependencies were updated.

## [1.23.5] - 2026-02-28

### Changed

- refactor: extract models to separate module #214
- refactor: extract popup and scroll modules from tui_app #217

## [1.23.4] - 2026-02-26

### Fixed

- fix: preserve key-only TXT records in service properties #213

## [1.23.3] - 2026-02-22

No user-facing changes.

## [1.23.2] - 2026-02-21

### Fixed

- fix: add Ctrl+Z to help popup on Unix systems #200

## [1.23.1] - 2026-02-21

### Added

- feat: add Ctrl+Z suspend support on Unix systems #198

### Changed

- refactor: extract terminal handling into own module #197

## [1.22.0] - 2026-02-17

### Added

- feat: add --load-state option to load and inspect state from JSON file #192

## [1.21.3] - 2026-02-15

Only dependencies were updated.

## [1.21.2] - 2026-02-15

### Added

- feat: add version to help popup title #187

## [1.21.1] - 2026-02-14

### Added

- feat: show online/offline/total counts in services list header #185

## [1.21.0] - 2026-02-14

### Added

- feat: add --no-ipv4 and --no-ipv6 CLI options to disable IPv4/IPv6 mDNS discovery #184

## [1.20.0] - 2026-02-13

### Added

- feat: add --interfaces CLI argument for interface selection #180

## [1.19.2] - 2026-02-13

No user-facing changes.

## [1.19.1] - 2026-02-12

No user-facing changes.

## [1.19.0] - 2026-02-12

### Added

- feat: add flapping service detection and visual styling #176

## [1.18.0] - 2026-02-12

### Added

- feat: extend quickfilter with online/offline special keywords #175

## [1.17.0] - 2026-02-11

### Added

- feat: add --no-debounce CLI option to disable flapping service debouncing #173

## [1.16.2] - 2026-02-11

Only dependencies were updated.

## [1.16.1] - 2026-02-10

No user-facing changes.

## [1.16.0] - 2026-02-10

### Added

- feat: enhance filter display with persistent bordered status #165

## [1.15.0] - 2026-02-10

### Added

- feat: implement 2-second service debouncing for flapping services #162

### Fixed

- fix: correct help popup color description for sort field #161
- fix: resolve race condition in ServiceResolved handling #163

## [1.14.12] - 2026-02-09

Only dependencies were updated.

## [1.13.5] - 2026-02-09

Only dependencies were updated.

## [1.13.4] - 2026-02-08

No user-facing changes.

## [1.13.3] - 2026-02-08

Only dependencies were updated.

## [1.13.2] - 2026-02-08

### Fixed

- fix: Correctly format service types with subtypes in display #132

## [1.13.1] - 2026-02-07

### Changed

- refactor: Deduplicate ServiceEntry creation in tests #129

## [1.13.0] - 2026-02-07

### Changed

- refactor: Remove redundant duration field from ServiceSession #128

### Fixed

- fix: Skip lastOnlineAt field if redundant with createdAt #126

## [1.12.1] - 2026-02-07

Only dependencies were updated.

## [1.12.0] - 2026-02-07

### Added

- feat: Add detailed timing tracking for mDNS services #122
- feat: Dump state to JSON with Ctrl+J #123

## [1.11.1] - 2026-02-04

### Fixed

- fix: remove underscore prefix from mDNS subtypes in compact format #116

## [1.11.0] - 2026-02-04

### Added

- feat: add scrolling to Service Details view #114

## [1.10.0] - 2026-02-04

### Added

- feat: enhance service type autocomplete with compact subtype support #113

## [1.9.2] - 2026-02-03

### Added

- feat: Allow subtypes via command line #104

### Changed

- refactor: Deduplicate scroll behavior across subviews #98

## [1.9.1] - 2026-02-03

### Added

- feat: Only browse user types if requested via cli #94

## [1.9.0] - 2026-02-02

### Added

- feat: Add CLI argument for additional service types #92

## [1.8.1] - 2026-02-01

### Fixed

- fix: Alignment of help controls #91

## [1.8.0] - 2026-02-01

### Added

- feat: Add paging and jump navigation for service types #89

## [1.7.3] - 2026-01-31

No user-facing changes.

## [1.7.2] - 2026-01-31

### Added

- feat: Ensure the service type is present if a service is discovered #85

## [1.7.1] - 2026-01-31

### Added

- feat: Add keyboard shortcut to clear stale service types #84

## [1.7.0] - 2026-01-31

### Changed

- refactor: rename remove_service to mark_service_offline and remove unused parameter #82

## [1.6.4] - 2026-01-31

### Fixed

- fix: service removal metric double counting #81

## [1.6.3] - 2026-01-31

No user-facing changes.

## [1.6.2] - 2026-01-30

### Fixed

- fix: prevent selection reset when clearing empty filter #74

## [1.6.1] - 2026-01-30

### Added

- feat: Add services_updated metric and improve service management #72

## [1.6.0] - 2026-01-30

### Added

- feat: Add GitHub build provenance and cargo auditable for releases #70

## [1.5.0] - 2026-01-30

### Added

- feat: Add sorting of services in services view #68
- feat: Quick filter functionality #69

## [1.4.1] - 2026-01-30

### Added

- feat: Add service lifetime tracking with timestamps #62
- feat: Comprehensive ServiceDaemon and application metrics #64

### Changed

- refactor: Rename dead/alive to offline/online #67

## [1.3.1] - 2026-01-29

### Fixed

- fix: Scroll the services view correctly after clearing dead services #59

## [1.3.0] - 2026-01-28

### Added

- feat: help popup and allow for clearing dead services #42

### Changed

- refactor: Move key handling code to AppState #43

### Fixed

- fix: Ignore key release events on windows to prevent duplicate events #41

## [1.2.1] - 2026-01-28

### Changed

- refactor: Major scrolling keybinding overhaul also handle Ctrl+C #40

## [1.2.0] - 2026-01-28

### Fixed

- fix: handle terminal resize events #36
- fix: Don't remove service types if those are still used #39

## [1.1.4] - 2026-01-28

Only dependencies were updated.

## [1.1.3] - 2026-01-28

No user-facing changes.

## [1.1.0] - 2026-01-27

No user-facing changes.

## [1.0.1] - 2026-01-27

### Added

- feat: remove open functionality #7
- feat: add vim like navigation #9
- feat: display all services by default #10
- feat: handle service type removal and service removal #13
- feat: Add host field and count indicators to services #18
- feat: blazingly fast caching and redundant information removal #11

### Changed

- refactor: Rename All Services to All Types for better clarity #20

### Fixed

- fix: Handle service type removal more thoroughly #21
- fix: Skip service subtypes in service type enumeration #22

[Unreleased]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.17...HEAD
[1.30.17]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.16...v1.30.17
[1.30.16]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.11...v1.30.16
[1.30.11]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.10...v1.30.11
[1.30.10]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.9...v1.30.10
[1.30.9]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.8...v1.30.9
[1.30.8]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.7...v1.30.8
[1.30.7]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.6...v1.30.7
[1.30.6]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.5...v1.30.6
[1.30.5]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.4...v1.30.5
[1.30.4]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.3...v1.30.4
[1.30.3]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.2...v1.30.3
[1.30.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.1...v1.30.2
[1.30.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.29.2...v1.30.1
[1.29.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.29.1...v1.29.2
[1.29.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.29.0...v1.29.1
[1.29.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.28.1...v1.29.0
[1.28.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.28.0...v1.28.1
[1.28.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.27.1...v1.28.0
[1.27.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.27.0...v1.27.1
[1.27.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.26.3...v1.27.0
[1.26.3]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.26.2...v1.26.3
[1.26.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.26.1...v1.26.2
[1.26.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.25.1...v1.26.1
[1.25.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.25.0...v1.25.1
[1.25.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.24.1...v1.25.0
[1.24.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.24.0...v1.24.1
[1.24.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.6...v1.24.0
[1.23.6]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.5...v1.23.6
[1.23.5]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.4...v1.23.5
[1.23.4]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.3...v1.23.4
[1.23.3]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.2...v1.23.3
[1.23.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.1...v1.23.2
[1.23.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.22.0...v1.23.1
[1.22.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.21.3...v1.22.0
[1.21.3]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.21.2...v1.21.3
[1.21.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.21.1...v1.21.2
[1.21.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.21.0...v1.21.1
[1.21.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.20.0...v1.21.0
[1.20.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.19.2...v1.20.0
[1.19.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.19.1...v1.19.2
[1.19.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.19.0...v1.19.1
[1.19.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.18.0...v1.19.0
[1.18.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.17.0...v1.18.0
[1.17.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.16.2...v1.17.0
[1.16.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.16.1...v1.16.2
[1.16.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.16.0...v1.16.1
[1.16.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.15.0...v1.16.0
[1.15.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.12...v1.15.0
[1.14.12]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.5...v1.14.12
[1.13.5]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.4...v1.13.5
[1.13.4]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.3...v1.13.4
[1.13.3]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.2...v1.13.3
[1.13.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.1...v1.13.2
[1.13.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.0...v1.13.1
[1.13.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.12.1...v1.13.0
[1.12.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.12.0...v1.12.1
[1.12.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.11.1...v1.12.0
[1.11.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.11.0...v1.11.1
[1.11.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.10.0...v1.11.0
[1.10.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.9.2...v1.10.0
[1.9.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.9.1...v1.9.2
[1.9.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.9.0...v1.9.1
[1.9.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.8.1...v1.9.0
[1.8.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.8.0...v1.8.1
[1.8.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.7.3...v1.8.0
[1.7.3]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.7.2...v1.7.3
[1.7.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.7.1...v1.7.2
[1.7.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.7.0...v1.7.1
[1.7.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.4...v1.7.0
[1.6.4]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.3...v1.6.4
[1.6.3]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.2...v1.6.3
[1.6.2]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.1...v1.6.2
[1.6.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.0...v1.6.1
[1.6.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.4.1...v1.5.0
[1.4.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.3.1...v1.4.1
[1.3.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.2.1...v1.3.0
[1.2.1]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.4...v1.2.0
[1.1.4]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.0...v1.1.3
[1.1.0]: https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/hrzlgnm/mdns-tui-browser/releases/tag/v1.0.1