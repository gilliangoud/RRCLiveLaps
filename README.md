# RRCLiveLaps
A simple web page showing live local lap times from a Race|Result decoder.

![RRCLiveLaps unconnected](unconnected.jpeg)

![RRCLiveLaps running](running.jpeg)


## Installation

This project is designed to be run as a standalone executable. It connects to the `rrconverter` service to receive live timing data.

### Prerequisites
1.  **RRConverter**: You must have `rrconverter` running to bridge the connection to your decoder. [Get it here](https://github.com/gilliangoud/rrconverter).
2.  **Mock Decoder (Optional)**: For testing without real hardware, run the `mock-decoder`.

## How to Run

### Option A: Distributed Executable
1.  Download the latest release for your platform (Windows/macOS/Linux).
2.  **Double-click** the executable (`rrclivelaps.exe` or `rrclivelaps`).
3.  Your default web browser will open automatically to the application.

### Option B: Build from Source
1.  Ensure you have Rust installed.
2.  Clone the repository and run:
    ```bash
    cd rrclivelaps
    cargo run
    ```
    To build a release binary:
    ```bash
    cargo build --release
    ```

## Customizing Transponder Names

You can map transponder IDs to custom driver names without recompiling the application.

1.  Create a file named `mapping.json` in the **same directory** as the `rrclivelaps` executable.
2.  Add your mappings in JSON format:
    ```json
    {
        "0000001": "Max Verstappen",
        "0000002": "Lewis Hamilton",
        "00000127": "Pace Car"
    }
    ```
3.  Restart `rrclivelaps`. The application will explicitly look for this file and load it; otherwise, it will fall back to its internal defaults.