# First car test (laptop)

Read only: nothing in this guide clears faults or changes the car.

## 1. Set up the laptop (at home)

1. Install Rust and git:

   ```bash
   sudo dnf install -y git gcc
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   source ~/.cargo/env
   ```

   (Debian/Ubuntu: `sudo apt install -y git build-essential` instead of `dnf`.)

2. Get the code and build it (takes a few minutes the first time):

   ```bash
   git clone https://github.com/TheOnlyLoki/bimmerscope ~/bimmerscope
   cd ~/bimmerscope
   cargo build --release -p bmwa -p obd --examples
   ```

3. Copy the ECU files (about 1.7 GB, not in git). From the desktop PC:

   ```bash
   rsync -a --info=progress2 ~/Downloads/de.holeschak.bmw_deep_obd/files/EcuRenamed/ <laptop>:ecu/
   ```

   Or copy the `EcuRenamed` folder with a USB stick to `~/ecu` on the laptop.
   Also copy `Configurations/Sample/obd.sim` to `~/ecu-sim/obd.sim` for the offline check.

4. Cable access: plug in the cable, then:

   ```bash
   printf '%s\n' 'SUBSYSTEM=="tty", ATTRS{idVendor}=="0403", ATTRS{idProduct}=="6001", TAG+="uaccess"' 'ACTION=="add", SUBSYSTEM=="usb-serial", DRIVER=="ftdi_sio", ATTR{latency_timer}="1"' | sudo tee /etc/udev/rules.d/70-kdcan.rules && sudo udevadm control --reload
   ```

   Unplug and replug the cable.

5. Offline check, no car needed:

   ```bash
   ./target/release/bmwa --ecu-dir ~/ecu --sim ~/ecu-sim/obd.sim
   ```

   Press `S`. You should see about 14 modules, "Park distance" with 1 fault.
   Press `4`: the Adapter tab should show the cable with green checks
   (latency 1 ms, access OK). Press `q` to quit.

## 2. In the car

1. Laptop on battery or charged. Cable switch on **K**.
2. Plug the cable into the OBD port (driver footwell, left of the steering column).
3. Ignition **on** (key position 2, dashboard lights on), engine **off**.
4. Run the test script:

   ```bash
   cd ~/bimmerscope
   tools/car_test.sh ~/ecu
   ```

   It runs IDENT on 5 modules and reads the fault memory of 2. Each line shows
   `exit 0` when the module answered. Takes up to 6 minutes when nothing answers.

5. If at least one module answered, try the app:

   ```bash
   ./target/release/bmwa --port /dev/ttyUSB0
   ```

   Press `S` and wait for the scan. Look at the faults. **Do not press `C`** (clear) yet.
   Take a photo or note what you see. Press `q` to quit.

6. Ignition off, unplug the cable.

Long tests drain the battery. Keep it under 30 minutes with the engine off.

## 3. Send the results

The script prints `Send this file: car-logs/<time>.tar.gz`. Copy that file
to the desktop PC (for example into `~/Projects/BMW_Analyzer/car-logs/`)
and say so in the chat.

## What errors mean

| Seen | Meaning |
|---|---|
| `IFH-0003`, "No echo received" | The cable does not echo bytes. Check the K/D-CAN switch. |
| `IFH-0009`, "No header" | The module did not answer. Ignition on? Switch on K? |
| `IFH-0018` | Cable not found or no access. Replug, check step 1.4. |
| `exit 124` | Timeout after 60 s. |
