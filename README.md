# bitslides

[![Test](https://github.com/debuti/bitslides/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/debuti/bitslides/actions/workflows/test.yml)
[![Coverage](https://codecov.io/gh/debuti/bitslides/branch/main/graph/badge.svg)](https://codecov.io/gh/debuti/bitslides)

Watches your volumes (HDD, USB drive, network mounted storage, etc.) and synchronizes the slides associated to them

<p align="center">
  <img src="https://github.com/debuti/bitslides/blob/dev/res/bitslides.webp?raw=true" alt="bitslides" width="200" height="200"  style="border:2px solid green;border-radius: 50px"/>
</p>

## Introduction

We all have multiple devices lying around: your personal computer, an external drive to save media, the smartphone, or even some cloud storage. Usually each device is dedicated to some specific tasks, the personal computer is used for photo editing, while the cloud storage is used to save your loved mp3s. Manually sending new contents to each device is a repetitive task that can be avoided using automation, and this is the gap `bitslides` solves, managing the transfer of information between devices.

Let's review its features through an example.

### Example Workflow with bitslides

*Donald* has
* a laptop (`laptop`), in which it rips DVDs into ISO files, but it also used for listening to music.
* a pendrive (`pen`), used for exchanging music with friends.
* a personal server (`server`), used for downloading his favourite TV shows, and also for transforming the ISOs into mp4s

Each time *Donald* needs to transfer files between his devices, he has to connect each device manually, organize the files into specific folders, and ensure no duplicates or misplaced files are left behind. This process can become tedious and error-prone, especially as the number of devices and file types grows.

This is where `bitslides` comes to the rescue. By configuring slides (mailboxes-like special directories associated with specific tasks or destinations) *Donald* can automate the entire workflow. Here's how `bitslides` simplifies *Donald*'s life:

#### Setup the slides

*Donald* configures `bitslides` on his laptop, pendrive, and personal server. For example:

On the laptop:
 * A slide for DVD ISO files (`/media/Laptop/Slides/Server/Movies/`).
 * A slide for music files (`/media/Pendrive/Slides/Laptop/Music/`).
On the server:
 * A slide for MP4s (`/media/Server/Slides/Laptop/Movies/`).

#### One shot behavior

> Daemon mode is first prio on the TO DO list

When ran `bitslides` automatically detects the attached volumes, checks for slides, and synchronizes the relevant files. For *Donald*:

 * When the pendrive is connected to the laptop, all music files in the pendrive are moved into the Laptop slide (`/media/Laptop/Slides/Laptop/Music/`).
 * When the server is mounted into the laptop:
   * all the ISOs are moved into the server slide (`/media/Server/Slides/Server/Movies/`)
   * all the MP4s created on the server are transferred to the laptop slide (`/media/Laptop/Slides/Laptop/Movies/`).

#### Default routes

Sometimes its not handy or even possible to have it all connected or mounted in the same computer. We can leverage the routing feature of `bitslides` to workaround this sort of situations. *Donald* decided to no longer mount the server drive for performance reasons, still he wants to send information from and to his Server

 * On the Laptop, create a `.slide.yml` in `/media/Laptop/Slides/Server/` and add these contents
 ```route: Pendrive```
 * On the Server, create a `.slide.yml` in `/media/Server/Slides/Laptop/` and add these contents
 ```route: Pendrive```

Now, each time *Donald* runs `bitslides` on the Laptop he will get the Server slide moved to `/media/Pendrive/Slides/Server/`, and when ran on the Server, the contents will end up arriving to its destination.

## Features

 * Device-Aware Synchronization
`bitslides` recognizes connected devices and their associated slides, avoiding unnecessary scans of unrelated volumes. On top of that, files are transferred only if the destination volume is available (e.g., mounted or online).

 * Multi-Protocol Support
Handles local storage, network-mounted drives, and even cloud-based file systems. Everything that it is mounted is compatible.

* Error Handling and Recovery
Robust mechanisms ensure incomplete transfers can resume seamlessly. Integrity of information is guaranteed by checksumming all the files before and after the copy.

* Cross-Platform
Runs on Linux, macOS, and Windows, ensuring compatibility across your devices.

* Powered by Rust
Leverages the performance and safety focus of the Rust language.

# Getting Started
To start using `bitslides`:

1. **Install**: Download and install `bitslides` from the [releases page](https://github.com/debuti/bitslides/releases).
2. **Configure**: Use the main configuration file (`bitslides.conf`) to define the places to look for synchable volumes. Create a `Slides` folder inside your volumes.
3. **Profit**: Launch bitslides and watch your devices stay perfectly synchronized without lifting a finger. Run `bitslides` with `-h` to learn more about the available options.

## Configuration

### Main config file

```
# roots: List of root folders where the software will look for volumes (synchable locations).
#  On Windows, every available logical drive will also be checked to be a volume
roots:
 - /media
 - /mnt

# keyword: Any synchable location has to contain a folder called as this keyword. The keyword defaults to "Slides".
keyword: "Queues"

# trace: Configure the software to write each event to a file.
trace: "bitslides.%Y%m%d_%H%M%S.log"
```

* `roots`: List of folders where the software will look for volumes (synchable locations).
* `keyword`: By default the folder that is going to be sync is named 'Slides' but you can override this name with this optional configuration.
* `trace`: Path or path template where the `bitslides` will save a record of the actions it took.


### Volume config file

```
# name: Name of the volume.
name: "myvolume"

# disabled: Opt-out of the sync for this volume.
#disabled: true
```

* `name`: Name override. By default the volume is named after the folder name, for example the volume `/media/Laptop/Slides` is named `Laptop`
* `disabled`: The volume is recognized but skipped for the sync process.

### Slide config file

```
# route:
route: "myothervol"
```

* `route`: Name of the volume you would want to use to approach to the final destination of this slide.


## Future Enhancements
 * **Real-Time Monitoring**: Continuous monitoring of changes to connected devices for immediate synchronization.

 * **Mobile App**: Companion app for Android/iOS to manage devices and monitor synchronization remotely.

## Contributions Welcome

`bitslides` is an open-source project! If you’d like to contribute, head over to the [GitHub repository](https://github.com/debuti/bitslides). Whether it’s fixing bugs, suggesting features, or improving documentation, your help is appreciated.

### Development environment

Installing pre-commit is recommended for saving CI time on the checks job

```bash
python3 -m pip install pre-commit
pre-commit install
```

### Release process

This project uses [cargo-dist](https://axodotdev.github.io/cargo-dist/book/) to publish releases.

```bash
# This will update your cargo-dist to the latest available version
cargo install cargo-dist --locked

# This will update the CI script that will run on
# * pull requests, where no release will happen
# * tags, that will create an actual release
dist init
#  Yes to update
#  Select the following platforms
#   [x] Apple Silicon macOS (aarch64-apple-darwin)
#   [x] ARM64 Linux (aarch64-unknown-linux-gnu)
#   [ ] ARM64 Windows (aarch64-pc-windows-msvc)
#   [x] Intel macOS (x86_64-apple-darwin)
#   [x] x64 Linux (x86_64-unknown-linux-gnu)
#   [x] x64 MUSL Linux (x86_64-unknown-linux-musl)
#   [x] x64 Windows (x86_64-pc-windows-msvc)
#  Select the following installers
#   [x] shell
#   [x] powershell
#   [ ] npm
#   [ ] homebrew
#   [x] msi

# Fix the generated CI script


# Actually push the tag up (this triggers dist's CI)
# * Make sure the version number matches the one configured in Cargo.toml
# * Only works on main
git tag v0.1.0
git push --tags
```
