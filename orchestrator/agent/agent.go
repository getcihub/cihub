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

	"github.com/containerd/containerd/errdefs"
	"github.com/firecracker-microvm/firecracker-go-sdk"
	"github.com/firecracker-microvm/firecracker-go-sdk/client/models"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
	"github.com/getcihub/cihub/orchestrator/manager"
	"github.com/getcihub/cihub/store/shared/db"
)

type Agent struct {
	sync.Mutex

	Manager   manager.RunnerManager
	Images    core.ImageService
	Snapshots core.SnapshotService

	Machine     string
	Firecracker string
	KernelArgs  string
	KernelPath  string

	Arch   string
	Memory int64
	Owner  string
	VCPU   int64
}

func (a *Agent) Run(ctx context.Context, job *core.Job) error {
	logger := logrus.WithFields(
		logrus.Fields{
			"installation_id": job.InstallationID,
			"job_id":          job.ID,
			"job_memory":      job.Memory,
			"job_vcpu":        job.VCPU,
		},
	)

	logger.Debugln("agent: get runner details from server")

	defer func() {
		// taking the paranoid approach to recover from
		// a panic that should absolutely never happen.
		if r := recover(); r != nil {
			logger.Errorf("agent: unexpected panic: %s", r)
			debug.PrintStack()
		}

		// Release agent's resource
		a.Lock()
		a.Memory += job.Memory
		a.VCPU += job.VCPU
		a.Unlock()
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

	runner, err := a.Manager.Register(ctx, job.ID)
	if err != nil {
		logger = logger.WithError(err).WithField("job_id", job.ID)
		logger.Warnln("agent: get runner details failed")
		return err
	}

	logger = logger.WithField("runner_name", runner.Name)
	imageExists, err := a.Images.Exists(ctx, job.OS)
	if err != nil {
		logger = logger.WithError(err).WithField("image", job.OS)
		logger.Errorln("agent: check image existence failed")
		return err
	}

	if !imageExists {
		logger.WithField("image", job.OS).Infoln("agent: pull image started")

		err = a.Images.Pull(ctx, job.OS)
		if err != nil {
			logger = logger.WithError(err).WithField("image", job.OS)
			logger.Errorln("agent: pull image failed")
			return err
		}

		logger.WithField("image", job.OS).Infoln("agent: pull image ok")
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

		snapshot, err = a.Snapshots.Create(ctx, runner.Name, job.OS)
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
			VcpuCount:  firecracker.Int64(job.VCPU),
			MemSizeMib: firecracker.Int64(job.Memory),
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
				"fireactions": map[string]interface{}{
					"runner_id":         runner.Name,
					"runner_jit_config": runner.Token,
				},
			},
		},
	}

	machine.Handlers.FcInit = machine.Handlers.FcInit.Append(firecracker.NewSetMetadataHandler(metadata))

	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	go func() {
		logger.WithField("runner_id", runner.ID).Debugln("agent: watch cancellation started")

		done, _ := a.Manager.Watch(ctx, runner.ID)
		if done {
			cancel()
			logger.WithField("runner_id", runner.ID).Debugln("agent: watch cancellation received")
		} else {
			logger.WithField("runner_id", runner.ID).Debugln("agent: watch cancellation finished")
		}
	}()

	// Notify manager that runner is starting
	if err := a.Manager.Started(ctx, runner.ID); err != nil {
		logger = logger.WithError(err).WithField("runner_id", runner.ID)
		logger.Warnln("agent: notify runner started failed")
		// Continue anyway—VM state is what matters, not notification
	}

	if err := machine.Start(ctx); err != nil {
		logger = logger.WithError(err).WithField("runner_name", runner.Name)
		logger.Errorln("agent: start VM failed")
		return err
	}

	logger.WithField("runner_name", runner.Name).Infoln("agent: start VM ok")

	err = machine.Wait(ctx)
	if err != nil {
		logger = logger.WithError(err).WithField("runner_name", runner.Name)
		logger.Warnln("agent: wait VM failed")
	}

	logger.WithField("runner_name", runner.Name).Debugln("agent: VM exited")

	// Notify manager that runner has completed
	status := core.RunnerStatusCompleted
	if ctx.Err() != nil {
		status = "cancelled" // Was cancelled during execution
	}

	if notifyErr := a.Manager.Completed(ctx, runner.ID, status); notifyErr != nil {
		logger = logger.WithError(notifyErr).WithField("runner_id", runner.ID).WithField("status", status)
		logger.Warnln("agent: notify runner completed failed")
		// Continue with cleanup regardless
	}

	shutdown, cancel := context.WithTimeout(context.Background(), time.Minute)
	defer cancel()

	err = a.Snapshots.Delete(shutdown, runner.Name)
	if err != nil && !errdefs.IsNotFound(err) {
		logger = logger.WithError(err).WithField("runner_name", runner.Name)
		logger.Errorln("agent: delete snapshot failed")
		return err
	} else {
		logger.WithField("runner_name", runner.Name).Debugln("agent: delete snapshot ok")
	}

	err = machineLogFile.Close()
	if err != nil {
		logger = logger.WithError(err).WithField("runner_name", runner.Name)
		logger.Warnln("agent: close machine log failed")
	}

	logger.WithField("runner_name", runner.Name).Infoln("agent: VM terminated")

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
			a.poll(ctx)
		}
	}
}

func (a *Agent) poll(ctx context.Context) error {
	log := logger.FromContext(ctx).
		WithFields(
			logrus.Fields{
				"machine":          a.Machine,
				"available_vcpu":   a.VCPU,
				"available_memory": a.Memory,
			},
		)
	log.Debugln("agent: poll queue started")

	// Call server and blocks until response or context cancellation
	job, err := a.Manager.Request(ctx, &core.Filter{
		Arch:   a.Arch,
		Memory: a.Memory,
		Owner:  a.Owner,
		VCPU:   a.VCPU,
	})

	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		log = log.WithError(err)
		log.Traceln("agent: request job timeout")
		return nil
	} else if err != nil {
		log = log.WithError(err)
		log.Errorln("agent: request job failed")
		return err
	}

	// exit if a nil or empty job is returned from the system
	// and allow the agent to retry.
	if job == nil || job.ID == 0 {
		return nil
	}

	log = log.WithFields(
		logrus.Fields{
			"job_id":     job.ID,
			"job_memory": job.Memory,
			"job_vcpu":   job.VCPU,
			"owner":      job.Owner,
			"repo":       job.Repo,
		},
	)

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	err = a.Manager.Accept(ctx, job.ID, a.Machine)
	if err == db.ErrOptimisticLock {
		return nil
	} else if err != nil {
		log = log.WithError(err)
		log.Warnln("agent: accept job failed")
		return err
	}

	a.Lock()
	a.Memory -= job.Memory
	a.VCPU -= job.VCPU
	a.Unlock()

	// Start microVM in a goroutine
	go a.Run(context.Background(), job)

	return nil
}
