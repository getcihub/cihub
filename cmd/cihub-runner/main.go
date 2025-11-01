package main

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"os/user"
	"path/filepath"
	"strconv"
	"syscall"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/service/metadata"
)

const (
	// defaultDir is the default directory where GitHub actions runner is stored
	defaultDir   = "/opt/runner"
	defaultOwner = "runner"
	defaultGroup = "docker"
)

func main() {
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, os.Kill)
	defer cancel()

	// Generate new metadata-service client
	mmds := metadata.New()

	// Fetch CIHub metadata from service
	md, err := mmds.Find(ctx, "cihub")
	if err != nil {
		logrus.WithError(err).Fatalln("runner: cannot get metadata")
	}

	if md.RunnerJITConfig == "" {
		logrus.Fatalln("runner: cannot get just-in-time configuration")
	}

	logrus.Infoln("Starting GitHub runner")

	cmd := exec.CommandContext(ctx, filepath.Join(defaultDir, "run.sh"), "--jitconfig", md.RunnerJITConfig)
	cmd.Dir = defaultDir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	owner, err := user.Lookup(defaultOwner)
	if err != nil {
		logrus.WithError(err).
			WithField("owner", defaultOwner).
			Fatalln("runner: cannot lookup owner")
	}

	uid, err := strconv.Atoi(owner.Uid)
	if err != nil {
		logrus.WithError(err).
			WithField("owner.uid", owner.Uid).
			Fatalln("runner: cannot convert owner UID")
	}

	group, err := user.LookupGroup(defaultGroup)
	if err != nil {
		logrus.WithError(err).
			WithField("group", defaultGroup).
			Fatalln("runner: cannot lookup group")
	}

	gid, err := strconv.Atoi(group.Gid)
	if err != nil {
		logrus.WithError(err).
			WithField("group.gid", group.Gid).
			Fatalln("runner: cannot convert group GID")
	}

	cmd.SysProcAttr = &syscall.SysProcAttr{Credential: &syscall.Credential{Uid: uint32(uid), Gid: uint32(gid)}}
	cmd.Env = append(cmd.Env,
		fmt.Sprintf("PATH=%s", "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"),
		fmt.Sprintf("LOGNAME=%s", owner.Username),
		fmt.Sprintf("HOME=%s", owner.HomeDir),
		fmt.Sprintf("USER=%s", owner.Username),
		fmt.Sprintf("UID=%d", uid),
		fmt.Sprintf("GID=%d", gid),
	)

	if err := cmd.Run(); err != nil {
		logrus.WithError(err).Fatalln("runner: program terminated")
	}
}
