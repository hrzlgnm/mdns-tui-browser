# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.32.2...HEAD)

### Dependencies

- *(deps)* Pin hugo19941994/delete-draft-releases action to 3f19a25 (#510) ([#510](https://github.com/hrzlgnm/mdns-tui-browser/pull/510))

- *(deps)* Lock file maintenance (#512) ([#512](https://github.com/hrzlgnm/mdns-tui-browser/pull/512))

- *(deps)* Update hrzlgnm/actions action to v2.5.5 (#514) ([#514](https://github.com/hrzlgnm/mdns-tui-browser/pull/514))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 2e59a58 (#515) ([#515](https://github.com/hrzlgnm/mdns-tui-browser/pull/515))

### Fixed

- *(release)* Fail on missing assets to upload to the release (#511) ([#511](https://github.com/hrzlgnm/mdns-tui-browser/pull/511))

## [1.32.2] - 2026-08-02 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.32.1...v1.32.2)

### Fixed

- *(release)* Delete draft releases with an action instead of gh (#509) ([#509](https://github.com/hrzlgnm/mdns-tui-browser/pull/509))

## [1.32.1] - 2026-08-02 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.32.0...v1.32.1)

### Fixed

- *(release)* Reorder checkout before download-artifact in source-checksums (#508) ([#508](https://github.com/hrzlgnm/mdns-tui-browser/pull/508))

## [1.32.0] - 2026-08-02 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.21...v1.32.0)

### Added

- *(release)* Publish drafts and upload assets with gh (#506) ([#506](https://github.com/hrzlgnm/mdns-tui-browser/pull/506))

### Changed

- *(ci)* Run changelog update on a nightly schedule (#494) ([#494](https://github.com/hrzlgnm/mdns-tui-browser/pull/494))

- Create changelog PR as verified github-actions bot (#497) ([#497](https://github.com/hrzlgnm/mdns-tui-browser/pull/497))

- Include dependencies in changelog (#503) ([#503](https://github.com/hrzlgnm/mdns-tui-browser/pull/503))

- Make the typos digest ignore pattern more broader (#505) ([#505](https://github.com/hrzlgnm/mdns-tui-browser/pull/505))

### Dependencies

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 0f536db (#487) ([#487](https://github.com/hrzlgnm/mdns-tui-browser/pull/487))

- *(deps)* Update hrzlgnm/actions action to v2.5.4 (#488) ([#488](https://github.com/hrzlgnm/mdns-tui-browser/pull/488))

- *(deps)* Update release-drafter/release-drafter action to v7.7.0 (#489) ([#489](https://github.com/hrzlgnm/mdns-tui-browser/pull/489))

- *(deps)* Update mozilla-actions/sccache-action action to v0.0.11 (#490) ([#490](https://github.com/hrzlgnm/mdns-tui-browser/pull/490))

- *(deps)* Update actions/attest digest to 508db95 (#491) ([#491](https://github.com/hrzlgnm/mdns-tui-browser/pull/491))

- *(deps)* Update rust crate clap to v4.6.5 (#492) ([#492](https://github.com/hrzlgnm/mdns-tui-browser/pull/492))

### Fixed

- Use PAT to create changelog PR (#498) ([#498](https://github.com/hrzlgnm/mdns-tui-browser/pull/498))

- Revert changelog PR to github-actions bot author (#500) ([#500](https://github.com/hrzlgnm/mdns-tui-browser/pull/500))

- *(release)* Delete pre-existing draft releases before drafting (#507) ([#507](https://github.com/hrzlgnm/mdns-tui-browser/pull/507))

## [1.30.21] - 2026-07-26 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.20...v1.30.21)

### Changed

- Move changelog comparison links inline with version sections (#481) ([#481](https://github.com/hrzlgnm/mdns-tui-browser/pull/481))

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v2.4.0 (#483) ([#483](https://github.com/hrzlgnm/mdns-tui-browser/pull/483))

- *(deps)* Update rust crate mdns-sd to v0.20.3 (#484) ([#484](https://github.com/hrzlgnm/mdns-tui-browser/pull/484))

- *(deps)* Lock file maintenance (#486) ([#486](https://github.com/hrzlgnm/mdns-tui-browser/pull/486))

## [1.30.20] - 2026-07-26 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.19...v1.30.20)

### Added

- Include changelog in all package formats (#477) ([#477](https://github.com/hrzlgnm/mdns-tui-browser/pull/477))

### Changed

- Ignore changelog update changes in release-drafter (#479) ([#479](https://github.com/hrzlgnm/mdns-tui-browser/pull/479))

## [1.30.19] - 2026-07-26 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.18...v1.30.19)

### Added

- Enable auto-merge for changelog PRs (#472) ([#472](https://github.com/hrzlgnm/mdns-tui-browser/pull/472))

### Changed

- Reuse data.tar.xz from .deb for .ipk packages (#460) ([#460](https://github.com/hrzlgnm/mdns-tui-browser/pull/460))

- Replace Python changelog script with git-cliff (#462) ([#462](https://github.com/hrzlgnm/mdns-tui-browser/pull/462))

- Add workflow to keep Unreleased changelog section up to date (#463) ([#463](https://github.com/hrzlgnm/mdns-tui-browser/pull/463))

### Fixed

- Use GH_ADMIN_TOKEN for changelog workflow PR creation (#464) ([#464](https://github.com/hrzlgnm/mdns-tui-browser/pull/464))

- Pass GH_ADMIN_TOKEN to checkout for git push permissions (#465) ([#465](https://github.com/hrzlgnm/mdns-tui-browser/pull/465))

- Use GH_CONTENT_WRITE token for changelog workflow (#467) ([#467](https://github.com/hrzlgnm/mdns-tui-browser/pull/467))

- Use manual signed commits for changelog workflow (#469) ([#469](https://github.com/hrzlgnm/mdns-tui-browser/pull/469))

- Check for open PR, not just any PR

- Use GH_CONTENT_WRITE for PR creation (#470) ([#470](https://github.com/hrzlgnm/mdns-tui-browser/pull/470))

- Skip changelog update commits in git-cliff (#476) ([#476](https://github.com/hrzlgnm/mdns-tui-browser/pull/476))

## [1.30.18] - 2026-07-26 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.17...v1.30.18)

### Added

- SBOM including attestation and grype scanning in release (#455) ([#455](https://github.com/hrzlgnm/mdns-tui-browser/pull/455))

- Integrate changelog generation into release workflow (#459) ([#459](https://github.com/hrzlgnm/mdns-tui-browser/pull/459))

### Changed

- Add changelog generation tooling (#454) ([#454](https://github.com/hrzlgnm/mdns-tui-browser/pull/454))

- Use crate cross for cross compilation and add more platforms (#458) ([#458](https://github.com/hrzlgnm/mdns-tui-browser/pull/458))

### Dependencies

- *(deps)* Update rust crate serde to v1.0.229 (#445) ([#445](https://github.com/hrzlgnm/mdns-tui-browser/pull/445))

- *(deps)* Update release-drafter/release-drafter action to v7.6.0 (#446) ([#446](https://github.com/hrzlgnm/mdns-tui-browser/pull/446))

- *(deps)* Update actions/checkout digest to 3d3c42e (#447) ([#447](https://github.com/hrzlgnm/mdns-tui-browser/pull/447))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to e5ae39e (#449) ([#449](https://github.com/hrzlgnm/mdns-tui-browser/pull/449))

- *(deps)* Update rust crate clap to v4.6.3 (#448) ([#448](https://github.com/hrzlgnm/mdns-tui-browser/pull/448))

- *(deps)* Update rust crate tokio to v1.53.1 (#451) ([#451](https://github.com/hrzlgnm/mdns-tui-browser/pull/451))

- *(deps)* Update actions/labeler action to v7 (#452) ([#452](https://github.com/hrzlgnm/mdns-tui-browser/pull/452))

- *(deps)* Update rust crate serde_json to v1.0.151 (#450) ([#450](https://github.com/hrzlgnm/mdns-tui-browser/pull/450))

- *(deps)* Update hrzlgnm/actions action to v2.3.1 (#453) ([#453](https://github.com/hrzlgnm/mdns-tui-browser/pull/453))

- *(deps)* Update anchore/sbom-action digest to e22c389 (#456) ([#456](https://github.com/hrzlgnm/mdns-tui-browser/pull/456))

- *(deps)* Update anchore/scan-action action to v7.4.0 (#457) ([#457](https://github.com/hrzlgnm/mdns-tui-browser/pull/457))

## [1.30.17] - 2026-07-17 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.16...v1.30.17)

### Changed

- Update workflow name to reflect the actual purpose (#435) ([#435](https://github.com/hrzlgnm/mdns-tui-browser/pull/435))

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v2.3.0 (#434) ([#434](https://github.com/hrzlgnm/mdns-tui-browser/pull/434))

- *(deps)* Update dependency cargo-edit to v0.13.12 (#436) ([#436](https://github.com/hrzlgnm/mdns-tui-browser/pull/436))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 66b197b (#437) ([#437](https://github.com/hrzlgnm/mdns-tui-browser/pull/437))

- *(deps)* Update dependency cargo-edit to v0.13.13 (#438) ([#438](https://github.com/hrzlgnm/mdns-tui-browser/pull/438))

- *(deps)* Update rust crate clap to v4.6.2 (#439) ([#439](https://github.com/hrzlgnm/mdns-tui-browser/pull/439))

- *(deps)* Update actions/attest digest to f7c74d2 (#440) ([#440](https://github.com/hrzlgnm/mdns-tui-browser/pull/440))

- *(deps)* Update dtolnay/rust-toolchain digest to 4cda84d (#441) ([#441](https://github.com/hrzlgnm/mdns-tui-browser/pull/441))

- *(deps)* Update rust crate tokio to v1.53.0 (#443) ([#443](https://github.com/hrzlgnm/mdns-tui-browser/pull/443))

- *(deps)* Update rust crate mdns-sd to v0.20.2 (#442) ([#442](https://github.com/hrzlgnm/mdns-tui-browser/pull/442))

- *(deps)* Lock file maintenance (#444) ([#444](https://github.com/hrzlgnm/mdns-tui-browser/pull/444))

## [1.30.15] - 2026-07-13 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.14...v1.30.15)

### Fixed

- Migrate version-resolver labels to version-resolver category syntax (#433) ([#433](https://github.com/hrzlgnm/mdns-tui-browser/pull/433))

## [1.30.14] - 2026-07-13 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.13...v1.30.14)

### Fixed

- Migrate exclude-labels to pre-exclude category syntax (#432) ([#432](https://github.com/hrzlgnm/mdns-tui-browser/pull/432))

## [1.30.13] - 2026-07-13 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.12...v1.30.13)

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v2.2.0 (#431) ([#431](https://github.com/hrzlgnm/mdns-tui-browser/pull/431))

### Fixed

- Migrate release-drafter categories to use when.labels syntax (#430) ([#430](https://github.com/hrzlgnm/mdns-tui-browser/pull/430))

## [1.30.12] - 2026-07-13 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.11...v1.30.12)

### Added

- Add daily retry workflow for failed CI on PRs (#424) ([#424](https://github.com/hrzlgnm/mdns-tui-browser/pull/424))

- Use retry-failed-ci reusable workflow (#429) ([#429](https://github.com/hrzlgnm/mdns-tui-browser/pull/429))

### Dependencies

- *(deps)* Lock file maintenance (#420) ([#420](https://github.com/hrzlgnm/mdns-tui-browser/pull/420))

- *(deps)* Update dependency cargo-nextest to v0.9.140 (#419) ([#419](https://github.com/hrzlgnm/mdns-tui-browser/pull/419))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 6258dc8 (#421) ([#421](https://github.com/hrzlgnm/mdns-tui-browser/pull/421))

- *(deps)* Update actions/labeler action to v6.2.0 (#422) ([#422](https://github.com/hrzlgnm/mdns-tui-browser/pull/422))

- *(deps)* Update rust crate open to v5.4.0 (#423) ([#423](https://github.com/hrzlgnm/mdns-tui-browser/pull/423))

- *(deps)* Lock file maintenance (#425) ([#425](https://github.com/hrzlgnm/mdns-tui-browser/pull/425))

- *(deps)* Update softprops/action-gh-release digest to 3d0d988 (#426) ([#426](https://github.com/hrzlgnm/mdns-tui-browser/pull/426))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 00160eb (#428) ([#428](https://github.com/hrzlgnm/mdns-tui-browser/pull/428))

### Fixed

- Only retry failed jobs (#427) ([#427](https://github.com/hrzlgnm/mdns-tui-browser/pull/427))

## [1.30.11] - 2026-07-05 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.10...v1.30.11)

### Dependencies

- *(deps)* Update rust crate mdns-sd to v0.20.1 (#413) ([#413](https://github.com/hrzlgnm/mdns-tui-browser/pull/413))

- *(deps)* Update dependency cargo-auditable to v0.7.5 (#414) ([#414](https://github.com/hrzlgnm/mdns-tui-browser/pull/414))

- *(deps)* Update rust crate open to v5.3.6 (#415) ([#415](https://github.com/hrzlgnm/mdns-tui-browser/pull/415))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 6b22aa4 (#416) ([#416](https://github.com/hrzlgnm/mdns-tui-browser/pull/416))

- *(deps)* Update dtolnay/rust-toolchain digest to 4be7066 (#417) ([#417](https://github.com/hrzlgnm/mdns-tui-browser/pull/417))

- *(deps)* Update dorny/paths-filter action to v4.0.2 (#418) ([#418](https://github.com/hrzlgnm/mdns-tui-browser/pull/418))

## [1.30.10] - 2026-06-26 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.9...v1.30.10)

### Dependencies

- *(deps)* Update rust crate ratatui to v0.30.2 (#403) ([#403](https://github.com/hrzlgnm/mdns-tui-browser/pull/403))

- *(deps)* Update softprops/action-gh-release digest to 718ea10 (#404) ([#404](https://github.com/hrzlgnm/mdns-tui-browser/pull/404))

- *(deps)* Update mikepenz/action-junit-report digest to d9f48fc (#405) ([#405](https://github.com/hrzlgnm/mdns-tui-browser/pull/405))

- *(deps)* Lock file maintenance (#407) ([#407](https://github.com/hrzlgnm/mdns-tui-browser/pull/407))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to cce7644 (#408) ([#408](https://github.com/hrzlgnm/mdns-tui-browser/pull/408))

- *(deps)* Update dependency cargo-nextest to v0.9.138 (#406) ([#406](https://github.com/hrzlgnm/mdns-tui-browser/pull/406))

- *(deps)* Update release-drafter/release-drafter action to v7.5.0 (#409) ([#409](https://github.com/hrzlgnm/mdns-tui-browser/pull/409))

- *(deps)* Update release-drafter/release-drafter action to v7.5.1 (#410) ([#410](https://github.com/hrzlgnm/mdns-tui-browser/pull/410))

- *(deps)* Update actions/attest digest to a1948c3 (#411) ([#411](https://github.com/hrzlgnm/mdns-tui-browser/pull/411))

- *(deps)* Lock file maintenance (#412) ([#412](https://github.com/hrzlgnm/mdns-tui-browser/pull/412))

## [1.30.9] - 2026-06-19 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.8...v1.30.9)

### Dependencies

- *(deps)* Update rust crate ratatui to v0.30.1 (#396) ([#396](https://github.com/hrzlgnm/mdns-tui-browser/pull/396))

- *(deps)* Lock file maintenance (#397) ([#397](https://github.com/hrzlgnm/mdns-tui-browser/pull/397))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to d73dfab (#398) ([#398](https://github.com/hrzlgnm/mdns-tui-browser/pull/398))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 40985c0 (#399) ([#399](https://github.com/hrzlgnm/mdns-tui-browser/pull/399))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 5011ed4 (#400) ([#400](https://github.com/hrzlgnm/mdns-tui-browser/pull/400))

- *(deps)* Update release-drafter/release-drafter action to v7.4.0 (#401) ([#401](https://github.com/hrzlgnm/mdns-tui-browser/pull/401))

- *(deps)* Update actions/checkout action to v7 (#402) ([#402](https://github.com/hrzlgnm/mdns-tui-browser/pull/402))

## [1.30.8] - 2026-06-04 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.7...v1.30.8)

### Dependencies

- *(deps)* Update rust crate serde_json to v1.0.150 (#380) ([#380](https://github.com/hrzlgnm/mdns-tui-browser/pull/380))

- *(deps)* Update hrzlgnm/actions action to v2.1.4 (#381) ([#381](https://github.com/hrzlgnm/mdns-tui-browser/pull/381))

- *(deps)* Lock file maintenance (#382) ([#382](https://github.com/hrzlgnm/mdns-tui-browser/pull/382))

- *(deps)* Update rust crate mdns-sd to 0.20 (#383) ([#383](https://github.com/hrzlgnm/mdns-tui-browser/pull/383))

- *(deps)* Update release-drafter/release-drafter action to v7.3.1 (#384) ([#384](https://github.com/hrzlgnm/mdns-tui-browser/pull/384))

- *(deps)* Update dependency cargo-nextest to v0.9.137 (#385) ([#385](https://github.com/hrzlgnm/mdns-tui-browser/pull/385))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to c326515 (#386) ([#386](https://github.com/hrzlgnm/mdns-tui-browser/pull/386))

- *(deps)* Update dependency cargo-edit to v0.13.11 (#387) ([#387](https://github.com/hrzlgnm/mdns-tui-browser/pull/387))

- *(deps)* Update hrzlgnm/actions action to v2.1.5 (#388) ([#388](https://github.com/hrzlgnm/mdns-tui-browser/pull/388))

- *(deps)* Lock file maintenance (#389) ([#389](https://github.com/hrzlgnm/mdns-tui-browser/pull/389))

- *(deps)* Update actions/checkout digest to df4cb1c (#390) ([#390](https://github.com/hrzlgnm/mdns-tui-browser/pull/390))

- *(deps)* Update rust crate chrono to v0.4.45 (#391) ([#391](https://github.com/hrzlgnm/mdns-tui-browser/pull/391))

- *(deps)* Update hrzlgnm/actions action to v2.1.6 (#394) ([#394](https://github.com/hrzlgnm/mdns-tui-browser/pull/394))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 4dea06f (#393) ([#393](https://github.com/hrzlgnm/mdns-tui-browser/pull/393))

- *(deps)* Lock file maintenance (#395) ([#395](https://github.com/hrzlgnm/mdns-tui-browser/pull/395))

### Fixed

- Prevent sed from corrupting arch stanza when updating sha256 (#392) ([#392](https://github.com/hrzlgnm/mdns-tui-browser/pull/392))

## [1.30.7] - 2026-05-19 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.6...v1.30.7)

### Dependencies

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 88792fc (#365) ([#365](https://github.com/hrzlgnm/mdns-tui-browser/pull/365))

- *(deps)* Update actions/labeler action to v6.1.0 (#367) ([#367](https://github.com/hrzlgnm/mdns-tui-browser/pull/367))

- *(deps)* Update release-drafter/release-drafter action to v7.3.0 (#368) ([#368](https://github.com/hrzlgnm/mdns-tui-browser/pull/368))

- *(deps)* Update rust crate tokio to v1.52.3 (#366) ([#366](https://github.com/hrzlgnm/mdns-tui-browser/pull/366))

- *(deps)* Lock file maintenance (#369) ([#369](https://github.com/hrzlgnm/mdns-tui-browser/pull/369))

- *(deps)* Update rust crate nix to v0.31.3 (#370) ([#370](https://github.com/hrzlgnm/mdns-tui-browser/pull/370))

- *(deps)* Update rust crate open to v5.3.5 (#371) ([#371](https://github.com/hrzlgnm/mdns-tui-browser/pull/371))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 4becbbb (#372) ([#372](https://github.com/hrzlgnm/mdns-tui-browser/pull/372))

- *(deps)* Update dependency cargo-nextest to v0.9.135 (#373) ([#373](https://github.com/hrzlgnm/mdns-tui-browser/pull/373))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 4e372d5 (#374) ([#374](https://github.com/hrzlgnm/mdns-tui-browser/pull/374))

- *(deps)* Update dependency cargo-nextest to v0.9.136 (#375) ([#375](https://github.com/hrzlgnm/mdns-tui-browser/pull/375))

- *(deps)* Update mikepenz/action-junit-report digest to 3a81627 (#376) ([#376](https://github.com/hrzlgnm/mdns-tui-browser/pull/376))

- *(deps)* Update rust crate mdns-sd to v0.19.2 (#377) ([#377](https://github.com/hrzlgnm/mdns-tui-browser/pull/377))

- *(deps)* Lock file maintenance (#378) ([#378](https://github.com/hrzlgnm/mdns-tui-browser/pull/378))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to b50db3b (#379) ([#379](https://github.com/hrzlgnm/mdns-tui-browser/pull/379))

## [1.30.6] - 2026-05-06 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.5...v1.30.6)

### Added

- Add homebrew tap release automation workflow (#361) ([#361](https://github.com/hrzlgnm/mdns-tui-browser/pull/361))

### Dependencies

- *(deps)* Lock file maintenance (#358) ([#358](https://github.com/hrzlgnm/mdns-tui-browser/pull/358))

- *(deps)* Update rust crate tokio to v1.52.2 (#359) ([#359](https://github.com/hrzlgnm/mdns-tui-browser/pull/359))

- *(deps)* Lock file maintenance (#360) ([#360](https://github.com/hrzlgnm/mdns-tui-browser/pull/360))

### Fixed

- Checkout tap repo within workspace directory (#362) ([#362](https://github.com/hrzlgnm/mdns-tui-browser/pull/362))

- Strip v prefix and correct sed patterns for homebrew workflow (#363) ([#363](https://github.com/hrzlgnm/mdns-tui-browser/pull/363))

- Use correct github-actions[bot] email and user (#364) ([#364](https://github.com/hrzlgnm/mdns-tui-browser/pull/364))

## [1.30.5] - 2026-05-03 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.4...v1.30.5)

### Dependencies

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to aa2b8b5 (#349) ([#349](https://github.com/hrzlgnm/mdns-tui-browser/pull/349))

- *(deps)* Update mozilla-actions/sccache-action action to v0.0.10 (#350) ([#350](https://github.com/hrzlgnm/mdns-tui-browser/pull/350))

- *(deps)* Lock file maintenance (#351) ([#351](https://github.com/hrzlgnm/mdns-tui-browser/pull/351))

- *(deps)* Update hrzlgnm/actions action to v2.1.2 (#352) ([#352](https://github.com/hrzlgnm/mdns-tui-browser/pull/352))

- *(deps)* Update dependency cargo-deb to v3.6.4 (#353) ([#353](https://github.com/hrzlgnm/mdns-tui-browser/pull/353))

- *(deps)* Update release-drafter/release-drafter action to v7.2.1 (#354) ([#354](https://github.com/hrzlgnm/mdns-tui-browser/pull/354))

- *(deps)* Update hrzlgnm/actions action to v2.1.3 (#355) ([#355](https://github.com/hrzlgnm/mdns-tui-browser/pull/355))

- *(deps)* Update dependency cargo-deb to v3.7.0 (#356) ([#356](https://github.com/hrzlgnm/mdns-tui-browser/pull/356))

### Fixed

- Ignore meta service as type when browsing (#357) ([#357](https://github.com/hrzlgnm/mdns-tui-browser/pull/357))

## [1.30.4] - 2026-04-20 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.3...v1.30.4)

### Changed

- Switch to actionlint from hrzlgnm/actions (#330) ([#330](https://github.com/hrzlgnm/mdns-tui-browser/pull/330))

- Update instructions to validate renovate config (#332) ([#332](https://github.com/hrzlgnm/mdns-tui-browser/pull/332))

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v2.1.0 (#331) ([#331](https://github.com/hrzlgnm/mdns-tui-browser/pull/331))

- *(deps)* Update softprops/action-gh-release digest to 3bb1273 (#333) ([#333](https://github.com/hrzlgnm/mdns-tui-browser/pull/333))

- *(deps)* Update softprops/action-gh-release action to v3 (#334) ([#334](https://github.com/hrzlgnm/mdns-tui-browser/pull/334))

- *(deps)* Lock file maintenance (#335) ([#335](https://github.com/hrzlgnm/mdns-tui-browser/pull/335))

- *(deps)* Update hrzlgnm/actions action to v2.1.1 (#336) ([#336](https://github.com/hrzlgnm/mdns-tui-browser/pull/336))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to d9bd9c7 (#337) ([#337](https://github.com/hrzlgnm/mdns-tui-browser/pull/337))

- *(deps)* Lock file maintenance (#338) ([#338](https://github.com/hrzlgnm/mdns-tui-browser/pull/338))

- *(deps)* Update dependency cargo-nextest to v0.9.133 (#339) ([#339](https://github.com/hrzlgnm/mdns-tui-browser/pull/339))

- *(deps)* Update rust crate tokio to v1.52.0 (#340) ([#340](https://github.com/hrzlgnm/mdns-tui-browser/pull/340))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 75337d6 (#341) ([#341](https://github.com/hrzlgnm/mdns-tui-browser/pull/341))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 9380162 (#342) ([#342](https://github.com/hrzlgnm/mdns-tui-browser/pull/342))

- *(deps)* Update rust crate clap to v4.6.1 (#343) ([#343](https://github.com/hrzlgnm/mdns-tui-browser/pull/343))

- *(deps)* Update rust crate tokio to v1.52.1 (#344) ([#344](https://github.com/hrzlgnm/mdns-tui-browser/pull/344))

- *(deps)* Update dependency cargo-edit to v0.13.10 (#345) ([#345](https://github.com/hrzlgnm/mdns-tui-browser/pull/345))

- *(deps)* Update rust crate open to v5.3.4 (#346) ([#346](https://github.com/hrzlgnm/mdns-tui-browser/pull/346))

- *(deps)* Lock file maintenance (#347) ([#347](https://github.com/hrzlgnm/mdns-tui-browser/pull/347))

- *(deps)* Update rust crate mdns-sd to v0.19.1 (#348) ([#348](https://github.com/hrzlgnm/mdns-tui-browser/pull/348))

## [1.30.3] - 2026-04-10 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.2...v1.30.3)

### Added

- Add DMG build support for macOS releases (#323) ([#323](https://github.com/hrzlgnm/mdns-tui-browser/pull/323))

### Changed

- Fix renovate config (#328) ([#328](https://github.com/hrzlgnm/mdns-tui-browser/pull/328))

### Dependencies

- *(deps)* Lock file maintenance (#315) ([#315](https://github.com/hrzlgnm/mdns-tui-browser/pull/315))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to efcda21 (#316) ([#316](https://github.com/hrzlgnm/mdns-tui-browser/pull/316))

- *(deps)* Update rust crate tokio to v1.51.1 (#317) ([#317](https://github.com/hrzlgnm/mdns-tui-browser/pull/317))

- *(deps)* Lock file maintenance (#318) ([#318](https://github.com/hrzlgnm/mdns-tui-browser/pull/318))

- *(deps)* Update release-drafter/release-drafter action to v7.2.0 (#320) ([#320](https://github.com/hrzlgnm/mdns-tui-browser/pull/320))

- *(deps)* Update actions/github-script action to v9 (#321) ([#321](https://github.com/hrzlgnm/mdns-tui-browser/pull/321))

- *(deps)* Update hrzlgnm/actions action to v2.0.7 (#322) ([#322](https://github.com/hrzlgnm/mdns-tui-browser/pull/322))

- *(deps)* Update actions/upload-artifact action to v7.0.1 (#324) ([#324](https://github.com/hrzlgnm/mdns-tui-browser/pull/324))

- *(deps)* Update actions/upload-artifact digest to 043fb46 (#325) ([#325](https://github.com/hrzlgnm/mdns-tui-browser/pull/325))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to be72b19 (#329) ([#329](https://github.com/hrzlgnm/mdns-tui-browser/pull/329))

### Fixed

- Speed up actionlint job by using native runner instead of Docker container (#326) ([#326](https://github.com/hrzlgnm/mdns-tui-browser/pull/326))

## [1.30.2] - 2026-04-05 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.1...v1.30.2)

### Changed

- Switch mdns-sd to published crate 0.19.0 (#314) ([#314](https://github.com/hrzlgnm/mdns-tui-browser/pull/314))

### Dependencies

- *(deps)* Update mdns-sd digest to b6ddc18 (#312) ([#312](https://github.com/hrzlgnm/mdns-tui-browser/pull/312))

- *(deps)* Update mdns-sd digest to d5f9060 (#313) ([#313](https://github.com/hrzlgnm/mdns-tui-browser/pull/313))

## [1.30.1] - 2026-04-04 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.30.0...v1.30.1)

### Added

- Limit services list to 5 items for larger details view (#311) ([#311](https://github.com/hrzlgnm/mdns-tui-browser/pull/311))

## [1.30.0] - 2026-04-04 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.29.2...v1.30.0)

### Added

- Set mdns ip check interval to 1s (#307) ([#307](https://github.com/hrzlgnm/mdns-tui-browser/pull/307))

- Add interface info to addresses (#309) ([#309](https://github.com/hrzlgnm/mdns-tui-browser/pull/309))

### Changed

- Enable coderabbit auto reviews (#308) ([#308](https://github.com/hrzlgnm/mdns-tui-browser/pull/308))

### Dependencies

- *(deps)* Update dependency cargo-nextest to v0.9.132 (#295) ([#295](https://github.com/hrzlgnm/mdns-tui-browser/pull/295))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 0483b22 (#296) ([#296](https://github.com/hrzlgnm/mdns-tui-browser/pull/296))

- *(deps)* Pin dtolnay/rust-toolchain action to 631a55b (#297) ([#297](https://github.com/hrzlgnm/mdns-tui-browser/pull/297))

- *(deps)* Update dtolnay/rust-toolchain digest to 29eef33 (#298) ([#298](https://github.com/hrzlgnm/mdns-tui-browser/pull/298))

- *(deps)* Update mikepenz/action-junit-report digest to bccf2e3 (#299) ([#299](https://github.com/hrzlgnm/mdns-tui-browser/pull/299))

- *(deps)* Update dependency komac to v2.16.0 (#300) ([#300](https://github.com/hrzlgnm/mdns-tui-browser/pull/300))

- *(deps)* Lock file maintenance (#301) ([#301](https://github.com/hrzlgnm/mdns-tui-browser/pull/301))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to b11278a (#302) ([#302](https://github.com/hrzlgnm/mdns-tui-browser/pull/302))

- *(deps)* Lock file maintenance (#303) ([#303](https://github.com/hrzlgnm/mdns-tui-browser/pull/303))

- *(deps)* Update hrzlgnm/actions action to v2.0.5 (#304) ([#304](https://github.com/hrzlgnm/mdns-tui-browser/pull/304))

- *(deps)* Update hrzlgnm/actions action to v2.0.6 (#305) ([#305](https://github.com/hrzlgnm/mdns-tui-browser/pull/305))

- *(deps)* Update rust crate tokio to v1.51.0 (#306) ([#306](https://github.com/hrzlgnm/mdns-tui-browser/pull/306))

## [1.29.2] - 2026-03-19 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.29.1...v1.29.2)

### Added

- Add IP-based URLs for HTTP services in get_urls() (#294) ([#294](https://github.com/hrzlgnm/mdns-tui-browser/pull/294))

### Dependencies

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 0d86f89 (#290) ([#290](https://github.com/hrzlgnm/mdns-tui-browser/pull/290))

- *(deps)* Update release-drafter/release-drafter action to v7.1.0 (#291) ([#291](https://github.com/hrzlgnm/mdns-tui-browser/pull/291))

- *(deps)* Update release-drafter/release-drafter action to v7.1.1 (#293) ([#293](https://github.com/hrzlgnm/mdns-tui-browser/pull/293))

- *(deps)* Update dependency cargo-nextest to v0.9.131 (#292) ([#292](https://github.com/hrzlgnm/mdns-tui-browser/pull/292))

## [1.29.1] - 2026-03-16 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.29.0...v1.29.1)

### Added

- Add URL selection popup for services with multiple URLs (#289) ([#289](https://github.com/hrzlgnm/mdns-tui-browser/pull/289))

## [1.29.0] - 2026-03-16 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.28.1...v1.29.0)

### Added

- Add Enter key to open service URL in browser (#287) ([#287](https://github.com/hrzlgnm/mdns-tui-browser/pull/287))

### Dependencies

- *(deps)* Update softprops/action-gh-release digest to 153bb8e (#288) ([#288](https://github.com/hrzlgnm/mdns-tui-browser/pull/288))

## [1.28.1] - 2026-03-15 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.28.0...v1.28.1)

### Fixed

- Spawn browser open in blocking task to avoid blocking event loop (#286) ([#286](https://github.com/hrzlgnm/mdns-tui-browser/pull/286))

## [1.28.0] - 2026-03-15 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.27.1...v1.28.0)

### Added

- Make service types panel width dynamic based on content (#285) ([#285](https://github.com/hrzlgnm/mdns-tui-browser/pull/285))

### Dependencies

- *(deps)* Update dorny/paths-filter action to v3.0.3 (#278) ([#278](https://github.com/hrzlgnm/mdns-tui-browser/pull/278))

- *(deps)* Update dorny/paths-filter action to v4 (#279) ([#279](https://github.com/hrzlgnm/mdns-tui-browser/pull/279))

- *(deps)* Update release-drafter/release-drafter action to v7 (#280) ([#280](https://github.com/hrzlgnm/mdns-tui-browser/pull/280))

- *(deps)* Update dorny/paths-filter action to v4.0.1 (#281) ([#281](https://github.com/hrzlgnm/mdns-tui-browser/pull/281))

- *(deps)* Update softprops/action-gh-release digest to 71d29a0 (#282) ([#282](https://github.com/hrzlgnm/mdns-tui-browser/pull/282))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 405a8aa (#283) ([#283](https://github.com/hrzlgnm/mdns-tui-browser/pull/283))

- *(deps)* Update softprops/action-gh-release digest to b25b93d (#284) ([#284](https://github.com/hrzlgnm/mdns-tui-browser/pull/284))

## [1.27.1] - 2026-03-12 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.27.0...v1.27.1)

### Dependencies

- *(deps)* Update rust crate clap to v4.6.0 (#276) ([#276](https://github.com/hrzlgnm/mdns-tui-browser/pull/276))

### Fixed

- Render error messages above help popup (#277) ([#277](https://github.com/hrzlgnm/mdns-tui-browser/pull/277))

## [1.27.0] - 2026-03-12 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.26.3...v1.27.0)

### Added

- Add keybind to open release notes from help popup (#273) ([#273](https://github.com/hrzlgnm/mdns-tui-browser/pull/273))

### Dependencies

- *(deps)* Update swatinem/rust-cache digest to c676846 (#272) ([#272](https://github.com/hrzlgnm/mdns-tui-browser/pull/272))

- *(deps)* Update swatinem/rust-cache digest to e18b497 (#274) ([#274](https://github.com/hrzlgnm/mdns-tui-browser/pull/274))

- *(deps)* Update rust crate clap to v4.5.61 (#275) ([#275](https://github.com/hrzlgnm/mdns-tui-browser/pull/275))

## [1.26.3] - 2026-03-11 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.26.2...v1.26.3)

### Changed

- Fix clone step in aur workflow (#271) ([#271](https://github.com/hrzlgnm/mdns-tui-browser/pull/271))

### Dependencies

- *(deps)* Update actions/download-artifact digest to 3e5f45b (#270) ([#270](https://github.com/hrzlgnm/mdns-tui-browser/pull/270))

## [1.26.2] - 2026-03-11 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.26.1...v1.26.2)

### Changed

- Simplify disabling interfaces when using specific ones (#267) ([#267](https://github.com/hrzlgnm/mdns-tui-browser/pull/267))

### Dependencies

- *(deps)* Update release-drafter/release-drafter action to v6.4.0 (#257) ([#257](https://github.com/hrzlgnm/mdns-tui-browser/pull/257))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 6bb7b3e (#261) ([#261](https://github.com/hrzlgnm/mdns-tui-browser/pull/261))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 0d1a4ba (#265) ([#265](https://github.com/hrzlgnm/mdns-tui-browser/pull/265))

- *(deps)* Update dependency cargo-nextest to v0.9.130 (#266) ([#266](https://github.com/hrzlgnm/mdns-tui-browser/pull/266))

- *(deps)* Update rust crate mdns-sd to v0.18.2 (#269) ([#269](https://github.com/hrzlgnm/mdns-tui-browser/pull/269))

### Fixed

- Address issues reported by actionlint (#259) ([#259](https://github.com/hrzlgnm/mdns-tui-browser/pull/259))

- Documentation block for run_tui (#268) ([#268](https://github.com/hrzlgnm/mdns-tui-browser/pull/268))

### Miscellaneous

- Add actionlint validation step to CI workflow (#262) ([#262](https://github.com/hrzlgnm/mdns-tui-browser/pull/262))

### Security

- Add explicit permissions to workflows and remove hardcoded ruleset id (#258) ([#258](https://github.com/hrzlgnm/mdns-tui-browser/pull/258))

## [1.26.1] - 2026-03-07 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.26.0...v1.26.1)

### Changed

- Cache cargo-auditable in cache-tools workflow too (#256) ([#256](https://github.com/hrzlgnm/mdns-tui-browser/pull/256))

## [1.26.0] - 2026-03-07 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.25.1...v1.26.0)

### Added

- Add flapping filter keyword (#255) ([#255](https://github.com/hrzlgnm/mdns-tui-browser/pull/255))

### Changed

- Cached install komac and cargo-edit using cargo-install (#247) ([#247](https://github.com/hrzlgnm/mdns-tui-browser/pull/247))

- Pass winget token via env to komac (#249) ([#249](https://github.com/hrzlgnm/mdns-tui-browser/pull/249))

- Cache-tools job after build (#251) ([#251](https://github.com/hrzlgnm/mdns-tui-browser/pull/251))

- Run lint and tests before cache-tools (#252) ([#252](https://github.com/hrzlgnm/mdns-tui-browser/pull/252))

- Extract user input handling to dedicated module (#254) ([#254](https://github.com/hrzlgnm/mdns-tui-browser/pull/254))

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v2.0.3 (#245) ([#245](https://github.com/hrzlgnm/mdns-tui-browser/pull/245))

- *(deps)* Update dependency cargo-auditable to v0.7.4 (#246) ([#246](https://github.com/hrzlgnm/mdns-tui-browser/pull/246))

- *(deps)* Update hrzlgnm/actions action to v2.0.4 (#250) ([#250](https://github.com/hrzlgnm/mdns-tui-browser/pull/250))

- *(deps)* Update release-drafter/release-drafter action to v6.3.0 (#253) ([#253](https://github.com/hrzlgnm/mdns-tui-browser/pull/253))

## [1.25.1] - 2026-03-04 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.25.0...v1.25.1)

### Changed

- Consolidate winget jobs to install komac only once (#240) ([#240](https://github.com/hrzlgnm/mdns-tui-browser/pull/240))

### Dependencies

- *(deps)* Update dependency cargo-auditable to v0.7.3 (#243) ([#243](https://github.com/hrzlgnm/mdns-tui-browser/pull/243))

### Fixed

- Calculate wrapped line count for popup scroll (#244) ([#244](https://github.com/hrzlgnm/mdns-tui-browser/pull/244))

## [1.25.0] - 2026-03-03 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.24.1...v1.25.0)

### Added

- Add ability to browse new service types at runtime (#239) ([#239](https://github.com/hrzlgnm/mdns-tui-browser/pull/239))

### Changed

- Collapse if conditions where applicable (#233) ([#233](https://github.com/hrzlgnm/mdns-tui-browser/pull/233))

- Agents should never suppress warnings (#235) ([#235](https://github.com/hrzlgnm/mdns-tui-browser/pull/235))

- Add unit test verifying flapping is recomputed on state load (#238) ([#238](https://github.com/hrzlgnm/mdns-tui-browser/pull/238))

### Dependencies

- *(deps)* Update dependency cargo-edit to v0.13.9 (#234) ([#234](https://github.com/hrzlgnm/mdns-tui-browser/pull/234))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to d06a3c6 (#236) ([#236](https://github.com/hrzlgnm/mdns-tui-browser/pull/236))

- *(deps)* Update rust crate tokio to v1.50.0 (#237) ([#237](https://github.com/hrzlgnm/mdns-tui-browser/pull/237))

## [1.24.1] - 2026-03-02 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.24.0...v1.24.1)

### Fixed

- Update services flapping status after loading a state dump (#231) ([#231](https://github.com/hrzlgnm/mdns-tui-browser/pull/231))

## [1.24.0] - 2026-03-02 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.6...v1.24.0)

### Added

- Remove --no-debounce option and improve flap detection (#230) ([#230](https://github.com/hrzlgnm/mdns-tui-browser/pull/230))

### Changed

- Increase collapse limit for dependency updates (#227) ([#227](https://github.com/hrzlgnm/mdns-tui-browser/pull/227))

- Regression test for non null interfaces round-trip (#229) ([#229](https://github.com/hrzlgnm/mdns-tui-browser/pull/229))

### Dependencies

- *(deps)* Update mikepenz/action-junit-report digest to 49b2ca0 (#225) ([#225](https://github.com/hrzlgnm/mdns-tui-browser/pull/225))

### Fixed

- Include CLI options in JSON state dump (#228) ([#228](https://github.com/hrzlgnm/mdns-tui-browser/pull/228))

## [1.23.6] - 2026-03-01 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.5...v1.23.6)

### Dependencies

- *(deps)* Update rust crate mdns-sd to v0.18.1 (#223) ([#223](https://github.com/hrzlgnm/mdns-tui-browser/pull/223))

- *(deps)* Lock file maintenance (#224) ([#224](https://github.com/hrzlgnm/mdns-tui-browser/pull/224))

## [1.23.5] - 2026-02-28 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.4...v1.23.5)

### Changed

- Extract models to separate module (#214) ([#214](https://github.com/hrzlgnm/mdns-tui-browser/pull/214))

- Extract popup and scroll modules from tui_app (#217) ([#217](https://github.com/hrzlgnm/mdns-tui-browser/pull/217))

### Dependencies

- *(deps)* Update github artifact actions (major) (#215) ([#215](https://github.com/hrzlgnm/mdns-tui-browser/pull/215))

- *(deps)* Update actions/attest-build-provenance digest to a2bbfa2 (#216) ([#216](https://github.com/hrzlgnm/mdns-tui-browser/pull/216))

- *(deps)* Update hrzlgnm/actions action to v2.0.2 (#219) ([#219](https://github.com/hrzlgnm/mdns-tui-browser/pull/219))

- *(deps)* Update actions/attest-build-provenance digest to c5efebd (#218) ([#218](https://github.com/hrzlgnm/mdns-tui-browser/pull/218))

- *(deps)* Update rust crate nix to v0.31.2 (#220) ([#220](https://github.com/hrzlgnm/mdns-tui-browser/pull/220))

- *(deps)* Pin actions/attest action to 59d8942 (#222) ([#222](https://github.com/hrzlgnm/mdns-tui-browser/pull/222))

### Miscellaneous

- Update attest-build-provenance to actions/attest@v4 (#221) ([#221](https://github.com/hrzlgnm/mdns-tui-browser/pull/221))

## [1.23.4] - 2026-02-26 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.3...v1.23.4)

### Changed

- Move manpage to docs directory (#202) ([#202](https://github.com/hrzlgnm/mdns-tui-browser/pull/202))

- Replace allowing dead code by conditional compilation (#203) ([#203](https://github.com/hrzlgnm/mdns-tui-browser/pull/203))

### Dependencies

- *(deps)* Update dependency cargo-nextest to v0.9.129 (#204) ([#204](https://github.com/hrzlgnm/mdns-tui-browser/pull/204))

- *(deps)* Update baptiste0928/cargo-install digest to f204293 (#205) ([#205](https://github.com/hrzlgnm/mdns-tui-browser/pull/205))

- *(deps)* Update rust crate chrono to v0.4.44 (#206) ([#206](https://github.com/hrzlgnm/mdns-tui-browser/pull/206))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 738111f (#207) ([#207](https://github.com/hrzlgnm/mdns-tui-browser/pull/207))

- *(deps)* Update mikepenz/action-junit-report digest to 5e05ac0 (#208) ([#208](https://github.com/hrzlgnm/mdns-tui-browser/pull/208))

- *(deps)* Update actions/attest-build-provenance digest to e4d4f7c (#211) ([#211](https://github.com/hrzlgnm/mdns-tui-browser/pull/211))

- *(deps)* Update actions/attest-build-provenance digest to 0856891 (#212) ([#212](https://github.com/hrzlgnm/mdns-tui-browser/pull/212))

### Fixed

- Preserve key-only TXT records in service properties (#213) ([#213](https://github.com/hrzlgnm/mdns-tui-browser/pull/213))

## [1.23.3] - 2026-02-22 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.2...v1.23.3)

### Changed

- Build for ubuntu-22.40, too (#201) ([#201](https://github.com/hrzlgnm/mdns-tui-browser/pull/201))

## [1.23.2] - 2026-02-21 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.1...v1.23.2)

### Fixed

- Add Ctrl+Z to help popup on Unix systems (#200) ([#200](https://github.com/hrzlgnm/mdns-tui-browser/pull/200))

## [1.23.1] - 2026-02-21 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.23.0...v1.23.1)

### Dependencies

- *(deps)* Lock file maintenance (#199) ([#199](https://github.com/hrzlgnm/mdns-tui-browser/pull/199))

## [1.23.0] - 2026-02-21 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.22.0...v1.23.0)

### Added

- Add Ctrl+Z suspend support on Unix systems (#198) ([#198](https://github.com/hrzlgnm/mdns-tui-browser/pull/198))

### Changed

- Extract terminal handling into own module (#197) ([#197](https://github.com/hrzlgnm/mdns-tui-browser/pull/197))

### Dependencies

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to d5b1842 (#193) ([#193](https://github.com/hrzlgnm/mdns-tui-browser/pull/193))

- *(deps)* Update rust crate clap to v4.5.60 (#194) ([#194](https://github.com/hrzlgnm/mdns-tui-browser/pull/194))

- *(deps)* Update dependency cargo-nextest to v0.9.128 (#196) ([#196](https://github.com/hrzlgnm/mdns-tui-browser/pull/196))

## [1.22.0] - 2026-02-17 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.21.3...v1.22.0)

### Added

- Add --load-state option to load and inspect state from JSON file (#192) ([#192](https://github.com/hrzlgnm/mdns-tui-browser/pull/192))

### Changed

- Add demo video to README (#189) ([#189](https://github.com/hrzlgnm/mdns-tui-browser/pull/189))

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v2.0.1 (#190) ([#190](https://github.com/hrzlgnm/mdns-tui-browser/pull/190))

- *(deps)* Update rust crate clap to v4.5.59 (#191) ([#191](https://github.com/hrzlgnm/mdns-tui-browser/pull/191))

## [1.21.3] - 2026-02-15 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.21.2...v1.21.3)

### Dependencies

- *(deps)* Update rust crate mdns-sd to 0.18 (#188) ([#188](https://github.com/hrzlgnm/mdns-tui-browser/pull/188))

## [1.21.2] - 2026-02-15 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.21.1...v1.21.2)

### Added

- Add version to help popup title (#187) ([#187](https://github.com/hrzlgnm/mdns-tui-browser/pull/187))

### Dependencies

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to ee97f53 (#186) ([#186](https://github.com/hrzlgnm/mdns-tui-browser/pull/186))

## [1.21.1] - 2026-02-14 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.21.0...v1.21.1)

### Added

- Show online/offline/total counts in services list header (#185) ([#185](https://github.com/hrzlgnm/mdns-tui-browser/pull/185))

## [1.21.0] - 2026-02-14 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.20.0...v1.21.0)

### Added

- Add --no-ipv4 and --no-ipv6 CLI options to disable IPv4/IPv6 mDNS discovery (#184) ([#184](https://github.com/hrzlgnm/mdns-tui-browser/pull/184))

### Dependencies

- *(deps)* Update dependency cargo-nextest to v0.9.127 (#183) ([#183](https://github.com/hrzlgnm/mdns-tui-browser/pull/183))

## [1.20.0] - 2026-02-13 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.19.2...v1.20.0)

### Added

- Add --interfaces CLI argument for interface selection (#180) ([#180](https://github.com/hrzlgnm/mdns-tui-browser/pull/180))

### Dependencies

- *(deps)* Update actions/attest-build-provenance digest to 02a49bd (#181) ([#181](https://github.com/hrzlgnm/mdns-tui-browser/pull/181))

- *(deps)* Update rust crate if-addrs to 0.15 (#182) ([#182](https://github.com/hrzlgnm/mdns-tui-browser/pull/182))

## [1.19.2] - 2026-02-13 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.19.1...v1.19.2)

### Changed

- Repackage .deb as .ipk for aarch64 and armhf (#179) ([#179](https://github.com/hrzlgnm/mdns-tui-browser/pull/179))

## [1.19.1] - 2026-02-12 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.19.0...v1.19.1)

### Changed

- Add .deb packages to Linux releases with attestation (#177) ([#177](https://github.com/hrzlgnm/mdns-tui-browser/pull/177))

## [1.19.0] - 2026-02-12 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.18.0...v1.19.0)

### Added

- Add flapping service detection and visual styling (#176) ([#176](https://github.com/hrzlgnm/mdns-tui-browser/pull/176))

## [1.18.0] - 2026-02-12 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.17.0...v1.18.0)

### Added

- Extend quickfilter with online/offline special keywords (#175) ([#175](https://github.com/hrzlgnm/mdns-tui-browser/pull/175))

### Changed

- Cargo set-version already updates Cargo.lock (#174) ([#174](https://github.com/hrzlgnm/mdns-tui-browser/pull/174))

## [1.17.0] - 2026-02-11 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.16.2...v1.17.0)

### Added

- Add --no-debounce CLI option to disable flapping service debouncing (#173) ([#173](https://github.com/hrzlgnm/mdns-tui-browser/pull/173))

### Dependencies

- *(deps)* Update rust crate clap to v4.5.58 (#171) ([#171](https://github.com/hrzlgnm/mdns-tui-browser/pull/171))

### Fixed

- Resolve actionlint warnings and extend documentation (#172) ([#172](https://github.com/hrzlgnm/mdns-tui-browser/pull/172))

## [1.16.2] - 2026-02-11 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.16.1...v1.16.2)

### Changed

- Publish to winget-pkgs on release (#117) ([#117](https://github.com/hrzlgnm/mdns-tui-browser/pull/117))

- Add winget installation instructions (#169) ([#169](https://github.com/hrzlgnm/mdns-tui-browser/pull/169))

### Dependencies

- *(deps)* Update actions/checkout digest to de0fac2 (#168) ([#168](https://github.com/hrzlgnm/mdns-tui-browser/pull/168))

### Fixed

- Ensure release build uses correct tag ref (#170) ([#170](https://github.com/hrzlgnm/mdns-tui-browser/pull/170))

## [1.16.1] - 2026-02-10 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.16.0...v1.16.1)

### Changed

- Update --service type option description (#166) ([#166](https://github.com/hrzlgnm/mdns-tui-browser/pull/166))

- Add link to desktop app (#167) ([#167](https://github.com/hrzlgnm/mdns-tui-browser/pull/167))

## [1.16.0] - 2026-02-10 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.15.0...v1.16.0)

### Added

- Enhance filter display with persistent bordered status (#165) ([#165](https://github.com/hrzlgnm/mdns-tui-browser/pull/165))

### Changed

- Cleanup debouncing and reduce debounce time to 1s (#164) ([#164](https://github.com/hrzlgnm/mdns-tui-browser/pull/164))

## [1.15.0] - 2026-02-10 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.13...v1.15.0)

### Added

- Implement 2-second service debouncing for flapping services (#162) ([#162](https://github.com/hrzlgnm/mdns-tui-browser/pull/162))

### Fixed

- Resolve race condition in ServiceResolved handling (#163) ([#163](https://github.com/hrzlgnm/mdns-tui-browser/pull/163))

## [1.14.13] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.12...v1.14.13)

### Fixed

- Correct help popup color description for sort field (#161) ([#161](https://github.com/hrzlgnm/mdns-tui-browser/pull/161))

## [1.14.12] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.11...v1.14.12)

### Fixed

- Permissions and input evaluation (#160) ([#160](https://github.com/hrzlgnm/mdns-tui-browser/pull/160))

## [1.14.11] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.10...v1.14.11)

### Dependencies

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to 1912107 (#156) ([#156](https://github.com/hrzlgnm/mdns-tui-browser/pull/156))

### Fixed

- Move reusable workflows to separate jobs (#158) ([#158](https://github.com/hrzlgnm/mdns-tui-browser/pull/158))

- Permissions of release workflow (#159) ([#159](https://github.com/hrzlgnm/mdns-tui-browser/pull/159))

## [1.14.10] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.9...v1.14.10)

### Fixed

- Add missing checkout steps to source-checksums workflow (#155) ([#155](https://github.com/hrzlgnm/mdns-tui-browser/pull/155))

## [1.14.9] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.8...v1.14.9)

### Added

- Consolidate publish workflow into release workflow (#154) ([#154](https://github.com/hrzlgnm/mdns-tui-browser/pull/154))

## [1.14.8] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.7...v1.14.8)

### Fixed

- Pass tagName as tag_name to action-gh-release (#153) ([#153](https://github.com/hrzlgnm/mdns-tui-browser/pull/153))

## [1.14.7] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.6...v1.14.7)

### Fixed

- Pass tagName to source-checksums-reusable workflow (#152) ([#152](https://github.com/hrzlgnm/mdns-tui-browser/pull/152))

## [1.14.6] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.5...v1.14.6)

### Fixed

- Add tagName input to workflow_call in source checksums workflow (#151) ([#151](https://github.com/hrzlgnm/mdns-tui-browser/pull/151))

## [1.14.5] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.4...v1.14.5)

### Fixed

- Use GITHUB_TOKEN instead of PAT (#150) ([#150](https://github.com/hrzlgnm/mdns-tui-browser/pull/150))

## [1.14.4] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.3...v1.14.4)

### Fixed

- Use proper permissions for workflow dispatch (#149) ([#149](https://github.com/hrzlgnm/mdns-tui-browser/pull/149))

## [1.14.3] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.2...v1.14.3)

### Fixed

- Add manual workflow trigger to bypass GitHub Actions limitation (#148) ([#148](https://github.com/hrzlgnm/mdns-tui-browser/pull/148))

## [1.14.1] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.14.0...v1.14.1)

### Fixed

- Perform cargo check --locked to avoid unintended updates (#145) ([#145](https://github.com/hrzlgnm/mdns-tui-browser/pull/145))

## [1.14.0] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.7...v1.14.0)

### Fixed

- Configure release-drafter to publish the release instead of drafting (#144) ([#144](https://github.com/hrzlgnm/mdns-tui-browser/pull/144))

## [1.13.7] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.6...v1.13.7)

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v2 (#142) ([#142](https://github.com/hrzlgnm/mdns-tui-browser/pull/142))

### Fixed

- Release-drafter configuration to create pre-releases instead of drafts (#143) ([#143](https://github.com/hrzlgnm/mdns-tui-browser/pull/143))

## [1.13.6] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.5...v1.13.6)

### Changed

- Replace bump-version and release-drafter workflows with unified release workflow (#140) ([#140](https://github.com/hrzlgnm/mdns-tui-browser/pull/140))

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v1.6.7 (#141) ([#141](https://github.com/hrzlgnm/mdns-tui-browser/pull/141))

## [1.13.5] - 2026-02-09 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.4...v1.13.5)

### Dependencies

- *(deps)* Update re-actors/alls-green digest to a638d64 (#138) ([#138](https://github.com/hrzlgnm/mdns-tui-browser/pull/138))

## [1.13.4] - 2026-02-08 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.3...v1.13.4)

### Changed

- Reduce metrics fetch interval from 5s to 1s (#137) ([#137](https://github.com/hrzlgnm/mdns-tui-browser/pull/137))

## [1.13.3] - 2026-02-08 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.2...v1.13.3)

### Dependencies

- *(deps)* Update re-actors/alls-green digest to b4ca9c2 (#135) ([#135](https://github.com/hrzlgnm/mdns-tui-browser/pull/135))

## [1.13.2] - 2026-02-08 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.1...v1.13.2)

### Changed

- Refactor tests (#133) ([#133](https://github.com/hrzlgnm/mdns-tui-browser/pull/133))

### Dependencies

- *(deps)* Update re-actors/alls-green digest to 3de8454 (#134) ([#134](https://github.com/hrzlgnm/mdns-tui-browser/pull/134))

### Fixed

- Correctly format service types with subtypes in display (#132) ([#132](https://github.com/hrzlgnm/mdns-tui-browser/pull/132))

## [1.13.1] - 2026-02-07 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.13.0...v1.13.1)

### Changed

- Align documentation with current CLI output and add date requirement (#130) ([#130](https://github.com/hrzlgnm/mdns-tui-browser/pull/130))

- Deduplicate ServiceEntry creation in tests (#129) ([#129](https://github.com/hrzlgnm/mdns-tui-browser/pull/129))

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v1.6.6 (#131) ([#131](https://github.com/hrzlgnm/mdns-tui-browser/pull/131))

## [1.13.0] - 2026-02-07 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.12.1...v1.13.0)

### Changed

- Don't scream the project name in man page (#127) ([#127](https://github.com/hrzlgnm/mdns-tui-browser/pull/127))

- Remove redundant duration field from ServiceSession (#128) ([#128](https://github.com/hrzlgnm/mdns-tui-browser/pull/128))

### Fixed

- Skip lastOnlineAt field if redundant with createdAt (#126) ([#126](https://github.com/hrzlgnm/mdns-tui-browser/pull/126))

## [1.12.1] - 2026-02-07 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.12.0...v1.12.1)

### Changed

- Update MSRV to 1.88 to allow for security updates (#124) ([#124](https://github.com/hrzlgnm/mdns-tui-browser/pull/124))

### Dependencies

- *(deps)* Lock file maintenance (#125) ([#125](https://github.com/hrzlgnm/mdns-tui-browser/pull/125))

## [1.12.0] - 2026-02-07 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.11.1...v1.12.0)

### Added

- Add detailed timing tracking for mDNS services (#122) ([#122](https://github.com/hrzlgnm/mdns-tui-browser/pull/122))

- Dump state to JSON with Ctrl+J (#123) ([#123](https://github.com/hrzlgnm/mdns-tui-browser/pull/123))

### Changed

- Add man pages (#119) ([#119](https://github.com/hrzlgnm/mdns-tui-browser/pull/119))

### Dependencies

- *(deps)* Update dependency cargo-nextest to v0.9.126 (#118) ([#118](https://github.com/hrzlgnm/mdns-tui-browser/pull/118))

- *(deps)* Update hrzlgnm/actions action to v1.6.5 (#120) ([#120](https://github.com/hrzlgnm/mdns-tui-browser/pull/120))

- *(deps)* Lock file maintenance (#121) ([#121](https://github.com/hrzlgnm/mdns-tui-browser/pull/121))

## [1.11.1] - 2026-02-04 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.11.0...v1.11.1)

### Fixed

- Remove underscore prefix from mDNS subtypes in compact format (#116) ([#116](https://github.com/hrzlgnm/mdns-tui-browser/pull/116))

## [1.11.0] - 2026-02-04 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.10.0...v1.11.0)

### Added

- Add scrolling to Service Details view (#114) ([#114](https://github.com/hrzlgnm/mdns-tui-browser/pull/114))

### Dependencies

- *(deps)* Update hrzlgnm/actions action to v1.6.4 (#115) ([#115](https://github.com/hrzlgnm/mdns-tui-browser/pull/115))

## [1.10.0] - 2026-02-04 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.9.2...v1.10.0)

### Added

- Enhance service type autocomplete with compact subtype support (#113) ([#113](https://github.com/hrzlgnm/mdns-tui-browser/pull/113))

### Changed

- Remove version extraction step from release drafter (#108) ([#108](https://github.com/hrzlgnm/mdns-tui-browser/pull/108))

- Configure version resolver in release drafter (#107) ([#107](https://github.com/hrzlgnm/mdns-tui-browser/pull/107))

- Remove Release Drafter trigger from bump version workflow (#110) ([#110](https://github.com/hrzlgnm/mdns-tui-browser/pull/110))

- Add 'enhancement' label to minor version releases (#111) ([#111](https://github.com/hrzlgnm/mdns-tui-browser/pull/111))

### Dependencies

- *(deps)* Update dependency cargo-nextest to v0.9.125 (#109) ([#109](https://github.com/hrzlgnm/mdns-tui-browser/pull/109))

- *(deps)* Update ghcr.io/hrzlgnm/mdns-browser-arch-aur-builder:v1 docker digest to ae41dd8 (#112) ([#112](https://github.com/hrzlgnm/mdns-tui-browser/pull/112))

## [1.9.2] - 2026-02-03 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.9.1...v1.9.2)

### Added

- Allow subtypes via command line (#104) ([#104](https://github.com/hrzlgnm/mdns-tui-browser/pull/104))

### Changed

- Clarify custom service discovery and examples (#97) ([#97](https://github.com/hrzlgnm/mdns-tui-browser/pull/97))

- Deduplicate scroll behavior across subviews (#98) ([#98](https://github.com/hrzlgnm/mdns-tui-browser/pull/98))

- Add AGENTS.md (#106) ([#106](https://github.com/hrzlgnm/mdns-tui-browser/pull/106))

### Dependencies

- *(deps)* Update actions/checkout digest to de0fac2 (#100) ([#100](https://github.com/hrzlgnm/mdns-tui-browser/pull/100))

- *(deps)* Update rust crate clap to v4.5.57 (#101) ([#101](https://github.com/hrzlgnm/mdns-tui-browser/pull/101))

- *(deps)* Lock file maintenance (#105) ([#105](https://github.com/hrzlgnm/mdns-tui-browser/pull/105))

## [1.9.1] - 2026-02-03 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.9.0...v1.9.1)

### Added

- Only browse user types if requested via cli (#94) ([#94](https://github.com/hrzlgnm/mdns-tui-browser/pull/94))

### Changed

- Remove superfluous sorting (#96) ([#96](https://github.com/hrzlgnm/mdns-tui-browser/pull/96))

### Dependencies

- *(deps)* Update actions/attest-build-provenance digest to c44148e (#93) ([#93](https://github.com/hrzlgnm/mdns-tui-browser/pull/93))

## [1.9.0] - 2026-02-02 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.8.1...v1.9.0)

### Added

- Add CLI argument for additional service types (#92) ([#92](https://github.com/hrzlgnm/mdns-tui-browser/pull/92))

## [1.8.1] - 2026-02-01 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.8.0...v1.8.1)

### Changed

- Clippy runs on tests, too (#90) ([#90](https://github.com/hrzlgnm/mdns-tui-browser/pull/90))

### Fixed

- Alignment of help controls (#91) ([#91](https://github.com/hrzlgnm/mdns-tui-browser/pull/91))

## [1.8.0] - 2026-02-01 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.7.3...v1.8.0)

### Added

- Add paging and jump navigation for service types (#89) ([#89](https://github.com/hrzlgnm/mdns-tui-browser/pull/89))

### Dependencies

- *(deps)* Lock file maintenance (#88) ([#88](https://github.com/hrzlgnm/mdns-tui-browser/pull/88))

## [1.7.3] - 2026-01-31 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.7.2...v1.7.3)

### Changed

- Disable stripping when building for AUR (#86) ([#86](https://github.com/hrzlgnm/mdns-tui-browser/pull/86))

## [1.7.2] - 2026-01-31 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.7.1...v1.7.2)

### Added

- Ensure the service type is present if a service is discovered (#85) ([#85](https://github.com/hrzlgnm/mdns-tui-browser/pull/85))

## [1.7.1] - 2026-01-31 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.7.0...v1.7.1)

### Added

- Add keyboard shortcut to clear stale service types (#84) ([#84](https://github.com/hrzlgnm/mdns-tui-browser/pull/84))

### Dependencies

- *(deps)* Update mikepenz/action-junit-report digest to 74626db (#83) ([#83](https://github.com/hrzlgnm/mdns-tui-browser/pull/83))

## [1.7.0] - 2026-01-31 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.4...v1.7.0)

### Changed

- Rename remove_service to mark_service_offline and remove unused parameter (#82) ([#82](https://github.com/hrzlgnm/mdns-tui-browser/pull/82))

## [1.6.4] - 2026-01-31 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.3...v1.6.4)

### Changed

- Ensure newline in AUR deploy key setup (#79) ([#79](https://github.com/hrzlgnm/mdns-tui-browser/pull/79))

- Add AUR installation instructions and version badge (#80) ([#80](https://github.com/hrzlgnm/mdns-tui-browser/pull/80))

### Fixed

- Service removal metric double counting (#81) ([#81](https://github.com/hrzlgnm/mdns-tui-browser/pull/81))

## [1.6.3] - 2026-01-31 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.2...v1.6.3)

### Changed

- Enforce safe Rust only policy (#75) ([#75](https://github.com/hrzlgnm/mdns-tui-browser/pull/75))

- Publish source checksums on release (#77) ([#77](https://github.com/hrzlgnm/mdns-tui-browser/pull/77))

- Deploy to AUR when release becomes latest (#76) ([#76](https://github.com/hrzlgnm/mdns-tui-browser/pull/76))

## [1.6.2] - 2026-01-30 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.1...v1.6.2)

### Changed

- Update Controls section in README.md (#73) ([#73](https://github.com/hrzlgnm/mdns-tui-browser/pull/73))

### Fixed

- Prevent selection reset when clearing empty filter (#74) ([#74](https://github.com/hrzlgnm/mdns-tui-browser/pull/74))

## [1.6.1] - 2026-01-30 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.6.0...v1.6.1)

### Added

- Add services_updated metric and improve service management (#72) ([#72](https://github.com/hrzlgnm/mdns-tui-browser/pull/72))

## [1.6.0] - 2026-01-30 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.5.0...v1.6.0)

### Added

- Add GitHub build provenance and cargo auditable for releases (#70) ([#70](https://github.com/hrzlgnm/mdns-tui-browser/pull/70))

### Dependencies

- *(deps)* Update actions/attest-build-provenance digest to 18db129 (#71) ([#71](https://github.com/hrzlgnm/mdns-tui-browser/pull/71))

## [1.5.0] - 2026-01-30 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.4.2...v1.5.0)

### Added

- Quick filter functionality (#69) ([#69](https://github.com/hrzlgnm/mdns-tui-browser/pull/69))

## [1.4.2] - 2026-01-30 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.4.1...v1.4.2)

### Added

- Add sorting of services in services view (#68) ([#68](https://github.com/hrzlgnm/mdns-tui-browser/pull/68))

## [1.4.1] - 2026-01-30 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.4.0...v1.4.1)

### Added

- Comprehensive ServiceDaemon and application metrics (#64) ([#64](https://github.com/hrzlgnm/mdns-tui-browser/pull/64))

### Changed

- Rename dead/alive to offline/online (#67) ([#67](https://github.com/hrzlgnm/mdns-tui-browser/pull/67))

## [1.4.0] - 2026-01-29 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.3.1...v1.4.0)

### Added

- Add service lifetime tracking with timestamps (#62) ([#62](https://github.com/hrzlgnm/mdns-tui-browser/pull/62))

### Dependencies

- *(deps)* Update rust crate clap to v4.5.56 (#61) ([#61](https://github.com/hrzlgnm/mdns-tui-browser/pull/61))

## [1.3.1] - 2026-01-29 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.3.0...v1.3.1)

### Changed

- Update documentation (#52) ([#52](https://github.com/hrzlgnm/mdns-tui-browser/pull/52))

- Add Badges to README (#54) ([#54](https://github.com/hrzlgnm/mdns-tui-browser/pull/54))

### Fixed

- Scroll the services view correctly after clearing dead services (#59) ([#59](https://github.com/hrzlgnm/mdns-tui-browser/pull/59))

## [1.3.0] - 2026-01-28 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.2.1...v1.3.0)

### Added

- Help popup and allow for clearing dead services (#42) ([#42](https://github.com/hrzlgnm/mdns-tui-browser/pull/42))

### Changed

- Move key handling code to AppState (#43) ([#43](https://github.com/hrzlgnm/mdns-tui-browser/pull/43))

### Fixed

- Ignore key release events on windows to prevent duplicate events (#41) ([#41](https://github.com/hrzlgnm/mdns-tui-browser/pull/41))

## [1.2.1] - 2026-01-28 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.2.0...v1.2.1)

### Changed

- Major scrolling keybinding overhaul also handle Ctrl+C (#40) ([#40](https://github.com/hrzlgnm/mdns-tui-browser/pull/40))

## [1.2.0] - 2026-01-28 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.5...v1.2.0)

### Changed

- Rename workflow to 'Reusable build workflow' (#37) ([#37](https://github.com/hrzlgnm/mdns-tui-browser/pull/37))

### Dependencies

- *(deps)* Lock file maintenance (#38) ([#38](https://github.com/hrzlgnm/mdns-tui-browser/pull/38))

### Fixed

- Don't remove service types if those are still used (#39) ([#39](https://github.com/hrzlgnm/mdns-tui-browser/pull/39))

## [1.1.5] - 2026-01-28 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.4...v1.1.5)

### Fixed

- Handle terminal resize events (#36) ([#36](https://github.com/hrzlgnm/mdns-tui-browser/pull/36))

## [1.1.4] - 2026-01-28 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.3...v1.1.4)

### Changed

- Only rerender if changes wer received (#34) ([#34](https://github.com/hrzlgnm/mdns-tui-browser/pull/34))

### Dependencies

- *(deps)* Lock file maintenance (#32) ([#32](https://github.com/hrzlgnm/mdns-tui-browser/pull/32))

- *(deps)* Update hrzlgnm/actions action to v1.6.3 (#33) ([#33](https://github.com/hrzlgnm/mdns-tui-browser/pull/33))

- *(deps)* Update rust crate flume to 0.12 (#35) ([#35](https://github.com/hrzlgnm/mdns-tui-browser/pull/35))

## [1.1.3] - 2026-01-28 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.2...v1.1.3)

### Changed

- Fix archiving debug symbols (#31) ([#31](https://github.com/hrzlgnm/mdns-tui-browser/pull/31))

## [1.1.2] - 2026-01-28 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.1...v1.1.2)

### Changed

- Split debug info (#30) ([#30](https://github.com/hrzlgnm/mdns-tui-browser/pull/30))

## [1.1.1] - 2026-01-27 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.1.0...v1.1.1)

### Changed

- Configure profile for release optimizations (#29) ([#29](https://github.com/hrzlgnm/mdns-tui-browser/pull/29))

## [1.1.0] - 2026-01-27 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.0.1...v1.1.0)

### Changed

- Cleanup naming of release assets and release name (#28) ([#28](https://github.com/hrzlgnm/mdns-tui-browser/pull/28))

## [1.0.1] - 2026-01-27 [compare](https://github.com/hrzlgnm/mdns-tui-browser/compare/v1.0.0...v1.0.1)

### Changed

- Simplify release publish workflow and update tag template (#27) ([#27](https://github.com/hrzlgnm/mdns-tui-browser/pull/27))

## [1.0.0] - 2026-01-27

### Added

- Update existing entries or push

- Make it cross compilable for aarch64

- Remove open functionality (#7) ([#7](https://github.com/hrzlgnm/mdns-tui-browser/pull/7))

- Add vim like navigation (#9) ([#9](https://github.com/hrzlgnm/mdns-tui-browser/pull/9))

- Display all services by default (#10) ([#10](https://github.com/hrzlgnm/mdns-tui-browser/pull/10))

- Blazingly fast caching and redundant information removal (#11) ([#11](https://github.com/hrzlgnm/mdns-tui-browser/pull/11))

- Handle service type removal and service removal (#13) ([#13](https://github.com/hrzlgnm/mdns-tui-browser/pull/13))

- Add host field and count indicators to services (#18) ([#18](https://github.com/hrzlgnm/mdns-tui-browser/pull/18))

### Changed

- Cargo clippy fix

- Steal renovate config from mdns-browser

- Rename project from mDNS Service Browser to mDNS TUI Browser (#5) ([#5](https://github.com/hrzlgnm/mdns-tui-browser/pull/5))

- Ci (#4) ([#4](https://github.com/hrzlgnm/mdns-tui-browser/pull/4))

- Configure coderabbit (#8) ([#8](https://github.com/hrzlgnm/mdns-tui-browser/pull/8))

- Temporary disable dependency dashboard (#14) ([#14](https://github.com/hrzlgnm/mdns-tui-browser/pull/14))

- Enable dependency dashboard again and remove unused extends (#15) ([#15](https://github.com/hrzlgnm/mdns-tui-browser/pull/15))

- Add multi-architecture build support to GitHub Actions (#17) ([#17](https://github.com/hrzlgnm/mdns-tui-browser/pull/17))

- Add ARM 32-bit cross-compilation support review (#19) ([#19](https://github.com/hrzlgnm/mdns-tui-browser/pull/19))

- Rename All Services to All Types for better clarity (#20) ([#20](https://github.com/hrzlgnm/mdns-tui-browser/pull/20))

- Add CI publishing workflows (#23) ([#23](https://github.com/hrzlgnm/mdns-tui-browser/pull/23))

- Extract version from Cargo.toml (#24) ([#24](https://github.com/hrzlgnm/mdns-tui-browser/pull/24))

- *(ci)* Attempt to fix bump version workflow (#25) ([#25](https://github.com/hrzlgnm/mdns-tui-browser/pull/25))

- Fix build-reusable workflow (#26) ([#26](https://github.com/hrzlgnm/mdns-tui-browser/pull/26))

### Dependencies

- *(deps)* Update rust crate crossterm to 0.29 (#2) ([#2](https://github.com/hrzlgnm/mdns-tui-browser/pull/2))

- *(deps)* Update rust crate ratatui to 0.30 (#3) ([#3](https://github.com/hrzlgnm/mdns-tui-browser/pull/3))

- *(deps)* Update rust crate crossterm to 0.29 (#12) ([#12](https://github.com/hrzlgnm/mdns-tui-browser/pull/12))

### Fixed

- Selecting items with mouse

- Handle service type removal more thoroughly (#21) ([#21](https://github.com/hrzlgnm/mdns-tui-browser/pull/21))

- Skip service subtypes in service type enumeration (#22) ([#22](https://github.com/hrzlgnm/mdns-tui-browser/pull/22))


