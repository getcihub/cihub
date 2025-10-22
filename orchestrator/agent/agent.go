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

func (a *Agent) Run(ctx context.Context, job *core.Job) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"cpu":             a.VCPU,
			"installation-id": job.InstallationID,
			"job-id":          job.ID,
			"memory":          a.Memory,
			"owner":           job.Owner,
			"repo":            job.Repo,
		},
	)

	logger.Debug("agent: get runner details from server")

	defer func() {
		// taking the paranoid approach to recover from
		// a panic that should absolutely never happen.
		if r := recover(); r != nil {
			logger.Errorf("agent: unexpected panic: %s", r)
			debug.PrintStack()
		}
	}()

	r, err := a.Manager.Details(ctx, job.ID)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("agent: cannot get runner details")
		return err
	}

	logger = logger.WithField("runner-name", r.Name)
	imageExists, err := a.Images.Exists(ctx, a.OS)
	if err != nil {
		logger = logger.WithError(err)
		logger.Errorln("agent: cannot check runner image existance")
		return err
	}

	if !imageExists {
		logger.Debugln("agent: image not found, pulling")

		err = a.Images.Pull(ctx, a.OS)
		if err != nil {
			logger = logger.WithError(err)
			logger.Errorln("agent: cannot pull runner image")
			return err
		}

		logger.Debugln("agent: image pulled")
	}

	snapshotExists, err := a.Snapshots.Exists(ctx, r.Name)
	if err != nil {
		logger = logger.WithError(err)
		logger.Errorln("agent: cannot check snapshot existance")
		return err
	}

	var snapshot *core.SnapshotMount
	if !snapshotExists {
		logger.Debugln("agent: snapshot not found, creating...")

		snapshot, err = a.Snapshots.Create(ctx, r.Name, a.OS)
		if err != nil {
			logger = logger.WithError(err)
			logger.Errorln("agent: cannot create snapshot")
			return err
		}

		logger.WithField("source", snapshot.Source).
			WithField("type", snapshot.Type).
			Debugln("agent: snapshot created")
	} else {
		logger.Debugln("agent: snapshot already exists")

		snapshot, err = a.Snapshots.Find(ctx, r.Name)
		if err != nil {
			logger = logger.WithError(err)
			logger.Errorln("agent: cannot find snapshot")
			return err
		}
	}

	machineLogFile, err := os.Create(filepath.Join("/var/lib/cihub", fmt.Sprintf("%s.log", r.Name)))
	if err != nil {
		logger.WithError(err).Fatal("Failed to create machine log file")
	}

	machineCmd := firecracker.VMCommandBuilder{}.
		WithStderr(machineLogFile).
		WithStdout(machineLogFile).
		WithSocketPath(filepath.Join("/var/lib/cihub", fmt.Sprintf("%s.sock", r.Name))).
		WithBin("/usr/local/bin/firecracker").
		Build(context.Background())

	firecrackerLogger := logrus.New()
	firecrackerLogger.SetLevel(logrus.WarnLevel)
	firecrackerLogger.SetOutput(io.Discard)

	machine, err := firecracker.NewMachine(ctx, firecracker.Config{
		VMID:            r.Name,
		SocketPath:      filepath.Join("/var/lib/cihub", fmt.Sprintf("%s.sock", r.Name)),
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
		MetricsPath:    filepath.Join("/tmp", fmt.Sprintf("%s.metrics", r.Name)),
	}, firecracker.WithProcessRunner(machineCmd), firecracker.WithLogger(logrus.NewEntry(firecrackerLogger)))

	if err != nil {
		logger = logger.WithError(err)
		logger.Errorln("agent: cannot create VM")
		return err
	}

	metadata := map[string]interface{}{
		"latest": map[string]interface{}{
			"meta-data": map[string]interface{}{
				"fireactions": map[string]interface{}{
					"runner_id":         r.Name,
					"runner_jit_config": r.Token,
				},
			},
		},
	}

	machine.Handlers.FcInit = machine.Handlers.FcInit.Append(firecracker.NewSetMetadataHandler(metadata))

	machineCtx, cancelMachine := context.WithCancel(context.Background())
	defer cancelMachine()

	go func() {
		logger.Debugln("agent: watch for cancel signal")

		done, _ := a.Manager.Watch(ctx, r.ID)
		if done {
			cancelMachine()
			logger.Debugln("agent: received cancel signal")
		} else {
			logger.Debugln("agent: done listening for cancel signals")
		}
	}()

	if err := machine.Start(ctx); err != nil {
		logger = logger.WithError(err)
		logger.Errorln("agent: cannot start runner VM")
		return err
	}

	logger.Infoln("agent: runner VM started, waiting for exit...")

	err = machine.Wait(machineCtx)
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("agent: cannot wait runner VM")
	}

	logger.Debugln("agent: microvm exited, releasing resources...")

	shutdown, cancel := context.WithTimeout(context.Background(), time.Minute)
	defer cancel()

	a.Snapshots.Delete(shutdown, r.Name)
	if err != nil && !errdefs.IsNotFound(err) {
		logger = logger.WithError(err)
		logger.Errorln("agent: cannot delete snapshot, need to be deleted manually")
		return err
	} else {
		logger.Debugln("agent: deleted snapshot")
	}

	err = machineLogFile.Close()
	if err != nil {
		logger = logger.WithError(err)
		logger.Warnln("agent: cannot close logs file")
	}

	logger.Infoln("agent: runner VM terminated")

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
		if !errors.Is(err, context.DeadlineExceeded) {
			logger = logger.WithError(err)
			logger.Warnln("agent: cannot get queue job")
		}

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

	_, err = a.Manager.Accept(ctx, job.ID, a.Machine)
	if err == db.ErrOptimisticLock {
		return nil
	} else if err != nil {
		logger.WithError(err).Warnln("agent: cannot ack job")
		return err
	}

	return a.Run(ctx, job)
}
