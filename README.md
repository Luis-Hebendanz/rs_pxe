# Rs_pxe
An 'all in one' command line PXE boot utility. Includes a PXE specific DHCP server and a TFTP server. It uses a RAW_SOCKET to talk to the network. Only supports Ethernet.


## Example Command
```bash
sudo ./result/bin/rs_pxe  --ipxe assets/ipxe.pxe -k assets/kernel.elf -i enp2s0 --raw
```
With debug logs:
```bash
sudo ./result/bin/rs_pxe -l DEBUG --ipxe assets/ipxe.pxe -k assets/kernel.elf -i enp2s0 --raw
```
To make the binary executable as a normal user. Execute the command below:
```bash
sudo setcap cap_net_admin,cap_net_raw=eip ./target/release/rs_pxe
```

## Install Dependencies
To drop into the development environment just execute:
```bash
./assets/nix-portable nix develop
```

OR

Install the [Nix package manager](https://nixos.org/download.html) by executing following command:
```bash
sh <(curl -L https://nixos.org/nix/install) --daemon --yes --nix-extra-conf-file ./assets/nix.conf && bash
```

## Development Environment
```bash
nix develop
```

Then to build the project:
```
cargo build
```
The resulting binary lies in: `target/debug/rs_pxe`

To run tests execute:
```
cargo test
```

You can use a wrapper shell script in the repo. It rebuilds & execute the binary on source changes:
```
./run.sh --none -i enp2s0
```

## Build Binary

To build an executable:
```bash
nix build
```
The resulting binary lies in `result/bin/rs_pxe`

