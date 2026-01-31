package agent

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"path/filepath"
	"runtime/debug"
	"time"

	"github.com/containerd/containerd/errdefs"
	"github.com/firecracker-microvm/firecracker-go-sdk"
	"github.com/firecracker-microvm/firecracker-go-sdk/client/models"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/client"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
	"github.com/getcihub/cihub/store/shared/db"
)

type Agent struct {
	Client    core.Client
	Images    core.ImageService
	Snapshots core.SnapshotService

	Firecracker string
	Image       string
	KernelArgs  string
	KernelPath  string
}

func (a *Agent) Run(ctx context.Context, runner *core.Runner) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"installation_id": runner.InstallationID,
			"runner_cpu":      runner.CPU,
			"runner_name":     runner.Name,
			"runner_ram":      runner.RAM,
		},
	)

	defer func() {
		// taking the paranoid approach to recover from
		// a panic that should absolutely never happen.
		if r := recover(); r != nil {
			logger.Errorf("agent: unexpected panic: %s", r)
			debug.PrintStack()
		}

		err := a.Client.Unlock(context.Background(), runner)
		if err != nil {
			logger.WithError(err).
				Errorln("agent: cannot unlock runner resources")
		}
	}()

	_, err := os.Stat("/var/lib/cihub")
	if os.IsNotExist(err) {
		logger.WithField("path", "/var/lib/cihub").Debugln("agent: create directory started")

		err = os.MkdirAll("/var/lib/cihub", 0755)
		if err != nil {
			logger := logger.WithError(err).WithField("path", "/var/lib/cihub")
			logger.Errorln("agent: create directory failed")
			return fmt.Errorf("agent: create directory failed: %w", err)
		}

		logger.WithField("path", "/var/lib/cihub").Debugln("agent: create directory ok")
	}

	imageExists, err := a.Images.Exists(ctx, a.Image)
	if err != nil {
		logger = logger.WithError(err).
			WithField("image", a.Image)
		logger.Errorln("agent: check image existence failed")
		return err
	}

	if !imageExists {
		logger.WithField("image", a.Image).Infoln("agent: pull image started")

		err = a.Images.Pull(ctx, a.Image)
		if err != nil {
			logger = logger.WithError(err).WithField("image", a.Image)
			logger.Errorln("agent: pull image failed")
			return err
		}

		logger.WithField("image", a.Image).Infoln("agent: pull image ok")
	}

	snapshotExists, err := a.Snapshots.Exists(ctx, runner.Name)
	if err != nil {
		logger = logger.WithError(err).WithField("runner_name", runner.Name)
		logger.Errorln("agent: check snapshot existence failed")
		return err
	}

	var snapshot *core.SnapshotMount
	if !snapshotExists {
		logger.WithField("runner_name", runner.Name).Debugln("agent: create snapshot started")

		snapshot, err = a.Snapshots.Create(ctx, runner.Name, a.Image)
		if err != nil {
			logger = logger.WithError(err).WithField("runner_name", runner.Name)
			logger.Errorln("agent: create snapshot failed")
			return err
		}

		logger.WithField("runner_name", runner.Name).
			WithField("source", snapshot.Source).
			WithField("type", snapshot.Type).
			Debugln("agent: create snapshot ok")
	} else {
		logger.WithField("runner_name", runner.Name).Debugln("agent: snapshot exists")

		snapshot, err = a.Snapshots.Find(ctx, runner.Name)
		if err != nil {
			logger = logger.WithError(err).WithField("runner_name", runner.Name)
			logger.Errorln("agent: find snapshot failed")
			return err
		}
	}

	machineLogFile, err := os.Create(filepath.Join("/var/lib/cihub", fmt.Sprintf("%s.log", runner.Name)))
	if err != nil {
		logger.WithError(err).WithField("runner_name", runner.Name).Fatalln("agent: create machine log file failed")
	}

	machineCmd := firecracker.VMCommandBuilder{}.
		WithStderr(machineLogFile).
		WithStdout(machineLogFile).
		WithSocketPath(filepath.Join("/var/lib/cihub", fmt.Sprintf("%s.sock", runner.Name))).
		WithBin(a.Firecracker).
		Build(context.Background())

	firecrackerLogger := logrus.New()
	firecrackerLogger.SetLevel(logrus.WarnLevel)
	firecrackerLogger.SetOutput(io.Discard)

	machine, err := firecracker.NewMachine(ctx, firecracker.Config{
		VMID:            runner.Name,
		SocketPath:      filepath.Join("/var/lib/cihub", fmt.Sprintf("%s.sock", runner.Name)),
		KernelImagePath: a.KernelPath,
		KernelArgs:      a.KernelArgs,
		MachineCfg: models.MachineConfiguration{
			MemSizeMib: firecracker.Int64(runner.RAM),
			VcpuCount:  firecracker.Int64(runner.CPU),
		},
		Drives: []models.Drive{{
			DriveID:      firecracker.String("rootfs"),
			PathOnHost:   &snapshot.Source,
			IsRootDevice: firecracker.Bool(true),
			IsReadOnly:   firecracker.Bool(false),
		}},
		NetworkInterfaces: []firecracker.NetworkInterface{{
			AllowMMDS:        true,
			CNIConfiguration: &firecracker.CNIConfiguration{NetworkName: "cihub", IfName: "eth0", ConfDir: "/etc/cni/net.d", BinPath: []string{"/opt/cni/bin"}},
		}},
		MmdsAddress:    net.IPv4(169, 254, 169, 254),
		MmdsVersion:    firecracker.MMDSv2,
		ForwardSignals: []os.Signal{},
		MetricsPath:    filepath.Join("/var/lib/cihub", fmt.Sprintf("%s.metrics", runner.Name)),
	}, firecracker.WithProcessRunner(machineCmd), firecracker.WithLogger(logrus.NewEntry(firecrackerLogger)))

	if err != nil {
		logger = logger.WithError(err).WithField("runner_name", runner.Name)
		logger.Errorln("agent: create VM failed")
		return err
	}

	metadata := map[string]interface{}{
		"latest": map[string]interface{}{
			"meta-data": map[string]interface{}{
				"cihub": map[string]interface{}{
					"runner_id":         runner.Name,
					"runner_jit_config": runner.Token,
				},
			},
		},
	}

	machine.Handlers.FcInit = machine.Handlers.FcInit.Append(firecracker.NewSetMetadataHandler(metadata))

	vmCtx, vmCancel := context.WithCancel(ctx)
	defer vmCancel()

	go func() {
		logger.Debugln("agent: watch cancellation started")

		done, _ := a.Client.Watch(ctx, runner)
		if done {
			vmCancel()
			logger.Debugln("agent: watch cancellation received")
		} else {
			logger.Debugln("agent: watch cancellation finished")
		}
	}()

	if err := machine.Start(vmCtx); err != nil {
		logger = logger.WithError(err)
		logger.Errorln("agent: start VM failed")
		return err
	}

	defer func() {
		if err := machine.StopVMM(); err != nil {
			logger = logger.WithError(err)
			logger.Errorln("agent: cannot stop firecracker VM")
		}
	}()

	logger.Infoln("agent: start VM ok")

	// Signal control-plane that the VM has started.
	if err := a.Client.Started(ctx, runner); err != nil {
		logger = logger.WithError(err)
		logger.Errorln("agent: cannot signal runner started")
		return err
	}

	// Wait for the VM to exit
	err = machine.Wait(vmCtx)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("agent: wait VM failed")
	} else {
		logger.Debugln("agent: VM exited")
	}

	shutdown, cancel := context.WithTimeout(context.Background(), time.Minute)
	defer cancel()

	err = a.Snapshots.Delete(shutdown, runner.Name)
	if err != nil && !errdefs.IsNotFound(err) {
		logger = logger.WithError(err)
		logger.Errorln("agent: delete snapshot failed")
		return err
	} else {
		logger.Debugln("agent: delete snapshot ok")
	}

	err = machineLogFile.Close()
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("agent: close machine log failed")
	}

	logger.Infoln("agent: VM terminated")

	return nil
}

// Start starts an agent process.
func (a *Agent) Start(ctx context.Context) error {
	for {
		select {
		case <-ctx.Done():
			return nil
		default:
			// This error is ignored on purpose. The system
			// should not exit the runner on error. The run
			// function logs all errors, which should be enough
			// to surface potential issues to an administrator.
			err := a.poll(ctx)

			// Exist if machine not registered
			if errors.Is(err, context.Canceled) {
				return nil
			}
		}
	}
}

func (a *Agent) poll(ctx context.Context) error {
	log := logger.FromContext(ctx)
	log.Debugln("agent: poll queue started")

	// Call server and blocks until response or context cancellation
	runner, err := a.Client.Request(ctx)

	// Check if agent is authorized
	if errors.Is(err, client.ErrMachineNotFound) {
		log.WithError(err).
			Infoln("agent: machine not registered on server, shutting down")
		return context.Canceled
	}

	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		log = log.WithError(err)
		log.Traceln("agent: request job timeout")
		return nil
	} else if err != nil {
		log = log.WithError(err)
		log.Errorln("agent: request job failed")
		return err
	}

	// exit if a nil runner is returned from the system
	// and allow the agent to retry.
	if runner == nil {
		return nil
	}

	log = log.WithField("runner_name", runner.Name)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	err = a.Client.Accept(ctx, runner.Runner)
	if err == db.ErrOptimisticLock {
		return nil
	} else if err != nil {
		log = log.WithError(err)
		log.Warnln("agent: cannot accept runner")
		return err
	}

	// Lock resources
	err = a.Client.Lock(ctx, runner.Runner)
	if err != nil {
		log = log.WithError(err)
		log.Warnln("agent: cannot lock runner resources")
		return err
	}

	// Start microVM in a goroutine
	go a.Run(context.Background(), runner.Runner)

	return nil
}
