#!/bin/bash

#=============================================================================
# CIHub Agent Installation Script
#
# This script installs and configures the CIHub agent with all its
# required dependencies including containerd, CNI plugins and Firecracker
#=============================================================================

set -o pipefail

#=============================================================================
# CONFIGURATION & ENVIRONMENT VARIABLES
#=============================================================================

# General
ARCH=amd64
DEFAULT_BRANCH="main"
DEFAULT_VERSION="latest"
INSTALL_PATH="/usr/local/bin"

# CIHub
CIHUB_AGENT_VERSION="${CIHUB_AGENT:=$DEFAULT_VERSION}"
CIHUB_AGENT_BIN="cihub-agent"
CIHUB_AGENT_REPO="getcihub/cihub"

# CNI Plugins
CNI_PLUGINS_VERSION="${CNI_PLUGINS:=$DEFAULT_VERSION}"
CNI_PLUGINS_REPO="containernetworking/plugins"
TC_REDIRECT_TAP_VERSION="${TC_REDIRECT_TAP:=$DEFAULT_VERSION}"
TC_REDIRECT_TAP_REPO="getcihub/tc-redirect-tap"

# Containerd
CONTAINERD_VERSION="${CONTAINERD:=$DEFAULT_VERSION}"
CONTAINERD_BIN="containerd"
CONTAINERD_REPO="containerd/containerd"
CONTAINERD_CONFIG_PATH="/etc/containerd/config.toml"
CONTAINERD_ROOT_DIR="/var/lib/containerd"
CONTAINERD_STATE_DIR="/run/containerd"
CONTAINERD_SYSTEMD_SVC="containerd.service"
CONTAINERD_SERVICE_FILE="/etc/systemd/system/containerd.service"
DEVMAPPER_DIR="$CONTAINERD_ROOT_DIR/devmapper"
DEVPOOL_DATA="$DEVMAPPER_DIR/data"
DEVPOOL_METADATA="$DEVMAPPER_DIR/metadata"

# Firecracker
FIRECRACKER_VERSION="${FIRECRACKER:=$DEFAULT_VERSION}"
FIRECRACKER_BIN="firecracker"
FIRECRACKER_REPO="firecracker-microvm/firecracker"

# Thinpool
THINPOOL_PROFILE_PATH="/etc/lvm/profile"
DEFAULT_THINPOOL="cihub"
DATA_SPARSE_SIZE="80G"
METADATA_SPARSE_SIZE="10G"
# Define thin-pool parameters.
# See https://www.kernel.org/doc/Documentation/device-mapper/thin-provisioning.txt for details.
SECTOR_SIZE=512
DATA_BLOCK_SIZE=128
# picked arbitrarily
# If free space on the data device drops below this level then a dm event will
# be triggered which a userspace daemon should catch allowing it to extend the
# pool device.
LOW_WATER_MARK=32768

#=============================================================================
# HELPER FUNCTIONS
#=============================================================================

# Send a green message to stdout
say_info() {
    echo -e "\033[0;32mINFO:\033[0m $1"
}

# Send a red message to stdout
say_err() {
    echo -e "\033[0;31mERROR:\033[0m $1"
}

# Send a yellow message to stdout
say_warn() {
    echo -e "\033[0;33mWARN:\033[0m $1"
}

# Exit with an error message and (optional) code
# Usage: die [-c <error code>] <error message>
die() {
    code=1
    [[ "$1" = "-c" ]] && {
        code="$2"
        shift 2
    }
    say_err "$@"
    exit "$code"
}

# Exit with an error message if the last exit code is not 0
ok_or_die() {
    code=$?
    [[ $code -eq 0 ]] || die -c $code "$@"
}

# Check if /dev/kvm exists. Exit if it doesn't.
ensure_kvm() {
    [[ -c /dev/kvm ]] || die "/dev/kvm not found. Required for virtualisation. Aborting."
}

#=============================================================================
# BUILDER FUNCTIONS
#=============================================================================

# Returns URL to latest release
build_release_url() {
    local repo_name="$1"
    echo "https://api.github.com/repos/$repo_name/releases/latest"
}

# Returns containerd release binary name
build_containerd_release_bin_name() {
    local tag=${1//v/} # remove the 'v' from arg $1
    local arch="$2"

    echo "containerd-$tag-linux-$arch.tar.gz"
}

# Returns the desired binary download url for a repo, tag and binary
build_download_url() {
    local repo_name="$1"
    local tag="$2"
    local bin="$3"
    echo "https://github.com/$repo_name/releases/download/$tag/$bin"
}

# Returns the URL to a raw github file
build_raw_url() {
    local repo_name="$1"
    local file_name="$2"
    echo "https://raw.githubusercontent.com/$repo_name/$DEFAULT_BRANCH/$file_name"
}

# Returns the tag associated with a "latest" release
latest_release_tag() {
    # shellcheck disable=SC2155
    local latest_url=$(build_release_url "$1")
    # shellcheck disable=SC2155
    local url=$(curl -sL "$latest_url" | awk -F'"' '/tag_name/ {printf $4}')
    echo "$url"
}

#=============================================================================
# DOER FUNCTIONS
#=============================================================================

# Checks user input for valid architecture and sets the global value for pulling
# correct binaries.
set_arch() {
    # shellcheck disable=SC2155
    local arch=$(uname -m)

    case $arch in
        x86_64 | amd64)
            ARCH=amd64
        ;;
        aarch64 | arm64)
            ARCH=arm64
        ;;
        *)
            die "Unknown arch or arch not supported: $arch."
        ;;
    esac
}

# Install and untar the tarred binary attached to a release to /usr/local/bin
install_release_tar() {
    local download_url="$1"
    local dest_path="$2"
    curl -sL "$download_url" | tar xz -C "$dest_path"
}

# Prepare the containerd state dirs
prepare_containerd_dirs() {
    local dirs=("$DEVMAPPER_DIR" "$CONTAINERD_STATE_DIR" "$(dirname $CONTAINERD_CONFIG_PATH)")
    for d in "${dirs[@]}"; do
        say_info "Creating containerd directory $d"

        mkdir -p "$d" || die "Failed to make containerd directory $d"
    done

    say_info "All containerd directories created"
}

# Download the given service file from the given repo
fetch_service_file() {
    local repo="$1"
    local service="$2"
    local dest="$3"
    # shellcheck disable=SC2155
    local url=$(build_raw_url "$repo" "$service")
    curl -o "$dest" -sL "$url" || die "failed to download $service"
    chmod 0664 "$dest"
    systemctl daemon-reload
}

# Enable and start the given systemd service
start_service() {
    local service="$1"
    systemctl enable "$service" &>/dev/null || die "failed to enable $service service"
    systemctl start "$service" || die "failed to start $service service"
}

#=============================================================================
# CNI PLUGINS
#=============================================================================

do_all_cni_plugins() {
    local cni_plugins_version="$1"
    local tc_redirect_tap_version="$2"

    install_cni_plugins "$cni_plugins_version"
    install_tc_redirect_tap "$tc_redirect_tap_version"
    write_cni_plugins_config
}

install_cni_plugins() {
    local tag="$1"
    local install_path="/opt/cni/bin"

    tempdir=$(mktemp -d)

    say_info "Install CNI plugins version $tag to $install_path"

    if [[ "$tag" == "$DEFAULT_VERSION" ]]; then
        tag=$(latest_release_tag "$CNI_PLUGINS_REPO")
    fi

    bin="cni-plugins-linux-${ARCH}-${tag}.tgz"
    url=$(build_download_url "$CNI_PLUGINS_REPO" "$tag" "$bin")
    curl -sL -o "$tempdir/$bin" "$url"
    mkdir -p "$install_path"
    tar -zxf "$tempdir/$bin" -C "$install_path"

    rm -rf "$tempdir"

    say_info "CNI plugins version $tag successfully installed"
}

install_tc_redirect_tap() {
    local tag="$1"
    local install_path="/opt/cni/bin"

    say_info "Install tc-redirect-tap CNI plugins version $tag to $install_path"

    if [[ "$tag" == "$DEFAULT_VERSION" ]]; then
        tag=$(latest_release_tag "$TC_REDIRECT_TAP_REPO")
    fi

    bin="tc-redirect-tap-linux-${ARCH}"
    url=$(build_download_url "$TC_REDIRECT_TAP_REPO" "$tag" "$bin")
    curl -sL -o "$install_path/tc-redirect-tap" "$url"
    chmod +x "$install_path/tc-redirect-tap"

    say_info "CNI plugin tc-redirect-tap version $tag successfully installed"
}

write_cni_plugins_config() {
    say_info "Writing CNI plugin configuration specific to CIHub"

    mkdir -p /etc/cni/net.d
    mkdir -p /var/run/cni

    cat <<EOF > /etc/cni/net.d/10-cihub.conflist
{
  "cniVersion": "0.4.0",
  "name": "cihub",
  "plugins": [
    {
      "type": "bridge",
      "bridge": "cihub-br0",
      "isDefaultGateway": true,
      "forceAddress": false,
      "ipMasq": true,
      "hairpinMode": true,
      "mtu": 1500,
      "ipam": {
        "type": "host-local",
        "subnet": "192.168.128.0/24",
        "resolvConf": "/etc/resolv.conf",
        "dataDir": "/var/run/cni"
      }
    },
    {
      "type": "firewall"
    },
    {
      "type": "tc-redirect-tap"
    }
  ]
}
EOF

    say_info "CNI plugins config saved"
}

#=============================================================================
# CONTAINERD
#=============================================================================

do_all_containerd() {
    local version="$1"
    local thinpool="$2"

    install_containerd "$version"
    write_containerd_config "$thinpool"
    start_containerd_service
}

install_containerd() {
    local tag="$1"

    say_info "Installing containerd version $tag to $INSTALL_PATH"

    if [[ "$version" == "$DEFAULT_VERSION" ]]; then
        tag=$(latest_release_tag "$CONTAINERD_REPO")
    fi

    bin=$(build_containerd_release_bin_name "$tag" "$ARCH")
    url=$(build_download_url "$CONTAINERD_REPO" "$tag" "$bin")
    install_release_tar "$url" "$(dirname $INSTALL_PATH)" || die "could not install containerd"

    "$CONTAINERD_BIN" --version &>/dev/null
    ok_or_die "Containerd version $tag not installed"

    say_info "Containerd version $tag successfully installed"
}

# Write out the containerd config file
write_containerd_config() {
    local thinpool="$1"

    say_info "Writing containerd config to $CONTAINERD_CONFIG_PATH"

    cat <<EOF >"$CONTAINERD_CONFIG_PATH"
version = 2

root = "$CONTAINERD_ROOT_DIR"
state = "$CONTAINERD_STATE_DIR"
imports   = []
oom_score = 0

[debug]
  level = "trace"

[grpc]
  address = "$CONTAINERD_STATE_DIR/containerd.sock"
  uid     = 0
  gid     = 0

[metrics]
  address = "127.0.0.1:1338"

[plugins]
  [plugins."io.containerd.snapshotter.v1.devmapper"]
    pool_name = "$thinpool-thinpool"
    root_path = "$DEVMAPPER_DIR"
    base_image_size = "30GB"
    discard_blocks = true
EOF

    say_info "Containerd config saved"
}

# Start the containerd systemd service
start_containerd_service() {
    say_info "Starting containerd service with $CONTAINERD_SERVICE_FILE"

    service="$CONTAINERD_BIN.service"
    fetch_service_file "$CONTAINERD_REPO" "$service" "$CONTAINERD_SERVICE_FILE"

    sed -i "s|ExecStart=.*|& --config $CONTAINERD_CONFIG_PATH|" "$CONTAINERD_SERVICE_FILE"

    start_service "$CONTAINERD_SYSTEMD_SVC"

    say_info "Containerd running"
}

#=============================================================================
# DEPENDENCIES
#=============================================================================

install_dependencies() {
    say_info "Installing required apt packages"
    apt update
    apt install -qq -y \
    --no-install-recommends \
        thin-provisioning-tools \
        e2fsprogs \
        e2fsck-static \
        git \
        build-essential \
        runc \
        bridge-utils \
        make \
        iptables \
        bc \
        lvm2 \
        dmsetup \
        pciutils \
        lm-sensors || die "failed to install apt packages"
    say_info "Packages installed"
}

#=============================================================================
# DEVPOOL
#=============================================================================

do_all_devpool() {
    local thinpool="$1-thinpool"

    say_info "Will create loop-back thinpool $thinpool"

    create_sparse_file "$DEVPOOL_DATA" "$DATA_SPARSE_SIZE"
    create_sparse_file "$DEVPOOL_METADATA" "$METADATA_SPARSE_SIZE"

    say_info "Associating loop devices with sparse files"

    datadev=$(associate_loop_device "$DEVPOOL_DATA")
    metadev=$(associate_loop_device "$DEVPOOL_METADATA")

    say_info "Loop devices $datadev and $metadev associated"

    create_dev_thinpool "$thinpool" "$datadev" "$metadev"

    say_info "Dev thinpool creation complete"
}

# Create a sparse file which will be used to back a loop device
create_sparse_file() {
    local file="$1"
    local size="$2"

    say_info "Creating sparse file $file of size $size"
    if [[ ! -f "$file" ]]; then
        touch "$file"
        truncate -s "$size" "$file" || die "Failed to create sparse file $file"
    fi

    say_info "Sparse file $file created"
}

# Assign a loop device to the given sparse file
associate_loop_device() {
    local sparse_file="$1"

    device="$(losetup --output NAME --noheadings --associated "$sparse_file")"
    if [[ -z "$device" ]]; then
        device=$(losetup --find --show "$sparse_file" || die "Failed to associate loop device with $sparse_file")
    fi

    echo "$device"
}

# Create the thinpool with the loop devices if it does not already exist
create_dev_thinpool() {
    local thinpool="$1"
    local datadev="$2"
    local metadev="$3"

    say_info "Creating thinpool $thinpool with devices $datadev and $metadev"

    datasize="$(blockdev --getsize64 -q "$datadev")"
    length_sectors=$(bc <<<"$datasize/$SECTOR_SIZE")
    thinp_table="0 $length_sectors thin-pool $metadev $datadev $DATA_BLOCK_SIZE $LOW_WATER_MARK 1 skip_block_zeroing"

    if ! dmsetup reload "$thinpool" --table "$thinp_table" 2>/dev/null; then
        dmsetup create "$thinpool" --table "$thinp_table" || die "failed to create dev thinpool $thinpool"
    fi

    say_info "Thinpool $thinpool created"
}

#=============================================================================
# FIRECRACKER
#=============================================================================

# Fetch and install the firecracker binary
install_firecracker() {
    local tag="$1"
    local arch=$(uname -m)

    tempdir=$(mktemp -d)

    say_info "Installing firecracker version $tag to $INSTALL_PATH"

    if [[ "$tag" == "$DEFAULT_VERSION" ]]; then
        tag=$(latest_release_tag "$FIRECRACKER_REPO")
    fi

    bin_name="${FIRECRACKER_BIN}-${tag}-${arch}.tgz"

    url=$(build_download_url "$FIRECRACKER_REPO" "$tag" "$bin_name")

    curl -sL "$url" | tar xz -C "$tempdir"

    fc_bin_down="$tempdir/release-$tag-$arch/firecracker-$tag-$arch"

    cp "$fc_bin_down" "$INSTALL_PATH/$FIRECRACKER_BIN"

    rm -rf "$tempdir"

    "$FIRECRACKER_BIN" --version &>/dev/null
    ok_or_die "firecracker version $tag not installed"

    say_info "Firecracker version $tag successfully installed"
}

#=============================================================================
# IP FORWARD
#=============================================================================

# Enable IPv4 packet forwarding on the system
enable_ip_forwarding() {
    say_info "Enabling IPv4 packet forwarding"

    cat <<EOF > /etc/sysctl.d/99-cihub.conf
net.ipv4.conf.all.forwarding=1
net.ipv4.ip_forward=1
EOF

    sysctl -p /etc/sysctl.d/99-cihub.conf > /dev/null || die "Failed to apply sysctl settings"

    say_info "IPv4 forwarding enabled"
}

#=============================================================================
# HELP COMMAND
#=============================================================================

help() {
    echo "Setup CIHub agent"
    echo
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --containerd-version                Specify the Containerd version to install                         (default: $CONTAINERD_VERSION)"
    echo "  --cni-plugins-version               Specify the CNI plugins version to install                        (default: $CNI_PLUGINS_VERSION)"
    echo "  --firecracker-version               Specify the Firecracker version to install                        (default: $FIRECRACKER_VERSION)"
    echo "  --tc-redirect-tap-version           Specify the CNI tc-redirect-tap plugin version to install         (default: $TC_REDIRECT_TAP_VERSION)"
    echo "  --disk                              Specify the disk for VM storage. If none specified, loop thinpools will be created."
    echo "  --skip-apt, -s                      Skip installation of apt packages"
    echo "  --thinpool, -t.                     Name of thinpool to create                                        (default: cihub or cihub-dev)"
}

#=============================================================================
# MAIN
#=============================================================================

main() {
    local containerd_version="$CONTAINERD_VERSION"
    local cni_plugins_version="$CNI_PLUGINS_VERSION"
    local firecracker_version="$FIRECRACKER_VERSION"
    local tc_redirect_tap_version="$TC_REDIRECT_TAP_VERSION"
    local disk=""
    local skip_apt=false
    local thinpool="$DEFAULT_THINPOOL"

    if [ "$(id -u)" -ne 0 ]; then
        die "CIHub agent installation script must be run as sudo or root"
    fi

    while [ $# -gt 0 ]; do
        case "$1" in
            "-h" | "--help")
                help
                exit 1
            ;;
            "--containerd-version")
                shift
                containerd_version="$1"
            ;;
            "--cni-plugins-version")
                shift
                cni_plugins_version="$1"
            ;;
            "--firecracker-version")
                shift
                firecracker_version="$1"
            ;;
            "--tc-redirect-tap-version")
                shift
                tc_redirect_tap_version="$1"
            ;;
            "-d" | "--disk")
                shift
                disk="$1"
            ;;
            "-s" | "--skip-apt")
                skip_apt=true
            ;;
            "-t" | "--thinpool")
                shift
                thinpool="$1"
            ;;
            *)
                die "Unknown argument: $1. Please use --help for help."
            ;;
        esac
        shift
    done

    say_info "Provisioning host $(hostname)"

    set_arch
    say_info "Will install binaries for architecture: $ARCH"

    ensure_kvm

    if [[ "$skip_apt" == false ]]; then
        install_dependencies
    fi

    prepare_containerd_dirs

    if [ -n "$disk" ]; then
        set_thinpool="${DEFAULT_THINPOOL:=$thinpool}"
    else
        do_all_devpool "$thinpool"
    fi

    do_all_cni_plugins "$cni_plugins_version" "$tc_redirect_tap_version"
    install_firecracker "$firecracker_version"
    do_all_containerd "$containerd_version" "$thinpool"
    enable_ip_forwarding

    say_info "Host $(hostname) provisioned"
}

main "$@"
