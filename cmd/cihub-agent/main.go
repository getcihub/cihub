package main

import (
	"context"
	"flag"
	"os/signal"
	"syscall"
	"time"

	"github.com/containerd/containerd"
	"github.com/sirupsen/logrus"
	"golang.org/x/sync/errgroup"

	"github.com/getcihub/cihub/agent"
	"github.com/getcihub/cihub/client"
	"github.com/getcihub/cihub/cmd/cihub-agent/config"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/pinger"
	"github.com/getcihub/cihub/service/image"
	"github.com/getcihub/cihub/service/resource"
	"github.com/getcihub/cihub/service/snapshot"
)

func main() {
	var confpath string
	flag.StringVar(&confpath, "c", "./config.toml", "Path to the configuration file")
	flag.Parse()

	config, err := config.Load(confpath)
	if err != nil {
		logrus.Fatalln(err)
	}

	// Set logging level
	logrus.SetLevel(config.Logger.Level)

	// Containerd must be installed on machine
	ctr, err := containerd.New(config.Agent.Containerd, containerd.WithDefaultNamespace("cihub"))
	if err != nil {
		logger := logrus.WithError(err)
		logger.Fatalln("agent: cannot create containerd client")
	}

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	manager := client.NewClient(config.RPC.Host, config.RPC.Secret)

	resourcez := resource.New()
	imagez := image.New(ctr, config.Agent.Snapshotter)
	snapshotz := snapshot.New(ctr, config.Agent.Snapshotter)
	pinger := pinger.New(manager, resourcez)
	agent := &agent.Agent{
		Client:      manager,
		Images:      imagez,
		Snapshots:   snapshotz,
		Firecracker: config.Agent.Firecracker,
		KernelArgs:  config.Agent.Kernel.Args,
		KernelPath:  config.Agent.Kernel.Path,
		Image:       config.Agent.Image,
	}

	resources, err := resourcez.Report(ctx)
	if err != nil {
		logger := logrus.WithError(err)
		logger.Fatalln("agent: cannot report machine resource")
	} else if resources.Arch == core.ArchUnknown {
		logrus.Fatalln("agent: unsupported CPU architecture")
	}

	// Ping the server and block until a successful connection
	// to the server has been established.
	for {
		err := manager.Ping(ctx, resources)
		select {
		case <-ctx.Done():
			return
		default:
		}

		if ctx.Err() != nil {
			break
		}

		if err != nil {
			logger := logrus.WithError(err)
			logger.Errorln("agent: cannot ping the remote server")
			time.Sleep(time.Second)
		} else {
			logrus.Infoln("agent: successfully pinged the remote server")
			break
		}
	}

	err = manager.Join(ctx)
	if err != nil {
		logrus.WithError(err).
			Fatalln("machine cannot join cluster")
	}

	var g errgroup.Group
	g.Go(func() error {
		logrus.WithField("arch", resources.Arch).
			WithField("cpu", resources.CPU).
			WithField("image", agent.Image).
			WithField("ram", resources.RAMAvailable).
			WithField("server", config.RPC.Host).
			Infoln("agent: start polling remote server")
		return agent.Start(ctx)
	})

	g.Go(func() error {
		return pinger.Start(ctx, time.Second*10)
	})

	if err := g.Wait(); err != nil {
		logrus.WithError(err).
			Fatalln("program terminated")
	}

	err = manager.Leave(context.Background())
	if err != nil {
		logrus.WithError(err).
			Warnln("machine cannot leave cluster")
	}
}
