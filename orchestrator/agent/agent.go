package agent

import (
	"context"
	"fmt"
	"io"
	"net"
	"os"
	"path/filepath"
	"sync"
	"time"

	"golang.org/x/sync/errgroup"

	"github.com/containerd/containerd/errdefs"
	"github.com/firecracker-microvm/firecracker-go-sdk"
	"github.com/firecracker-microvm/firecracker-go-sdk/client/models"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/orchestrator/manager"
	"github.com/getcihub/cihub/store/shared/db"
)

type Agent struct {
	sync.Mutex

	Manager   manager.RunnerManager
	Images    core.ImageService
	Snapshots core.SnapshotService

	Kernel  string
	Labels  []string
	Machine string
	Memory  int64
	OS      string
	VCPU    int64
}

func (a *Agent) Run(ctx context.Context, runner *core.Runner) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"runner.name": runner.Name,
			"runner.id":   runner.ID,
			"vcpu":        a.VCPU,
			"memory":      a.Memory,
		},
	)

	imageExists, err := a.Images.Exists(ctx, a.OS)
	if err != nil {
		logger.WithError(err).
			Errorln("agent: failed to check image existance")
		return err
	}

	if !imageExists {
		logger.Debugln("agent: image not found, pulling")

		err = a.Images.Pull(ctx, a.OS)
		if err != nil {
			logger.WithError(err).
				Errorln("agent: failed to pull image")
			return err
		}

		logger.Debugln("agent: image pulled")
	}

	snapshotExists, err := a.Snapshots.Exists(ctx, runner.Name)
	if err != nil {
		logger.WithError(err).
			Errorln("agent: failed to check snapshot existance")
		return err
	}

	var snapshot *core.SnapshotMount
	if !snapshotExists {
		logger.Debugln("agent: snapshot not found, creating...")

		snapshot, err = a.Snapshots.Create(ctx, runner.Name, a.OS)
		if err != nil {
			logger.WithError(err).
				Errorln("agent: failed to create snapshot")
			return err
		}

		logger.WithField("source", snapshot.Source).
			WithField("type", snapshot.Type).
			Debugln("agent: snapshot created")
	} else {
		logger.Debugln("agent: snapshot already exists")

		snapshot, err = a.Snapshots.Find(ctx, runner.Name)
		if err != nil {
			logger.WithError(err).
				Errorln("agent: failed to get snapshot")
			return err
		}
	}

	machineLogFile, err := os.Create(filepath.Join("/tmp", fmt.Sprintf("%s.log", runner.Name)))
	if err != nil {
		logger.WithError(err).Fatal("Failed to create machine log file")
	}

	machineCmd := firecracker.VMCommandBuilder{}.
		WithStderr(machineLogFile).
		WithStdout(machineLogFile).
		WithSocketPath(filepath.Join("/tmp", fmt.Sprintf("%s.sock", runner.Name))).
		WithBin("/usr/local/bin/firecracker").
		Build(context.Background())

	firecrackerLogger := logrus.New()
	firecrackerLogger.SetLevel(logrus.WarnLevel)
	firecrackerLogger.SetOutput(io.Discard)

	machine, err := firecracker.NewMachine(ctx, firecracker.Config{
		VMID:            runner.Name,
		SocketPath:      filepath.Join("/tmp", fmt.Sprintf("%s.sock", runner.Name)),
		KernelImagePath: a.Kernel,
		KernelArgs:      "console=ttyS0 reboot=k panic=1 pci=off nomodules rw",
		MachineCfg: models.MachineConfiguration{
			VcpuCount:  firecracker.Int64(a.VCPU),
			MemSizeMib: firecracker.Int64(a.Memory),
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
		MetricsPath:    filepath.Join("/tmp", fmt.Sprintf("%s.metrics", runner.Name)),
	}, firecracker.WithProcessRunner(machineCmd), firecracker.WithLogger(logrus.NewEntry(firecrackerLogger)))

	if err != nil {
		logger = logger.WithError(err)
		logger.Errorln("agent: failed to create VM")
		return err
	}

	metadata := map[string]interface{}{
		"latest": map[string]interface{}{
			"meta-data": map[string]interface{}{
				"fireactions": map[string]interface{}{
					"runner_id":         runner.Name,
					"runner_jit_config": runner.Token,
				},
			},
		},
	}

	machine.Handlers.FcInit = machine.Handlers.FcInit.Append(firecracker.NewSetMetadataHandler(metadata))

	if err := machine.Start(context.Background()); err != nil {
		return fmt.Errorf("agent: failed starting runner, err: %w", err)
	}

	logger.Infoln("agent: microvm started, waiting for exit...")

	err = machine.Wait(context.Background())
	if err != nil {
		logger.WithError(err).
			Warnln("agent: error while waiting for VM exit")
	}

	logger.Debugln("agent: microvm exited, releasing resources...")

	shutdown, cancel := context.WithTimeout(context.Background(), time.Second*5)
	defer cancel()

	a.Snapshots.Delete(shutdown, runner.Name)
	if err != nil && !errdefs.IsNotFound(err) {
		logger.WithError(err).
			Errorln("agent: failed to remove containerd lease, need to release it manually")
	}

	err = machineLogFile.Close()
	if err != nil {
		logger.WithError(err).Warnln("agent: failed to close log")
	}

	logger.Infoln("agent: microvm exited")

	return nil
}

// Start starts N runner agent processes. Each process polls
// the server for pending runners to execute.
func (a *Agent) Start(ctx context.Context, n int) error {
	var g errgroup.Group
	for i := 0; i < n; i++ {
		g.Go(func() error {
			return a.start(ctx, i)
		})
	}
	return g.Wait()
}

func (a *Agent) start(ctx context.Context, thread int) error {
	for {
		select {
		case <-ctx.Done():
			return nil
		default:
			// This error is ignored on purpose. The system
			// should not exit the runner on error. The run
			// function logs all errors, which should be enough
			// to surface potential issues to an administrator.
			a.poll(ctx, thread)
		}
	}
}

func (a *Agent) poll(ctx context.Context, thread int) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"machine": a.Machine,
			"labels":  a.Labels,
			"thread":  thread,
		},
	)

	logger.Debugln("agent: polling queue")
	job, err := a.Manager.Request(ctx, a.Labels)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("agent: cannot get queue job")
		return err
	}

	if job == nil || job.ID == 0 {
		return nil
	}

	logger = logger.WithFields(
		logrus.Fields{
			"job.id": job.ID,
			"owner":  job.Owner,
			"repo":   job.Repo,
			"run.id": job.RunID,
		},
	)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	runner, err := a.Manager.Accept(ctx, job.ID, a.Machine)
	if err == db.ErrOptimisticLock {
		return nil
	} else if err != nil {
		logger.WithError(err).Warnln("agent: cannot ack job")
		return err
	}

	go func() {
		logger.Debugln("agent: watch for cancel signal")
		done, _ := a.Manager.Watch(ctx, runner.ID)
		if done {
			cancel()
			logger.Debugln("agent: received cancel signal")
		} else {
			logger.Debugln("agent: done listening for cancel signals")
		}
	}()

	return a.Run(ctx, runner)
}
