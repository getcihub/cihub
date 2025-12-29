package scheduler

import (
	"context"
	"math/rand"
	"sync"
	"testing"
	"time"

	"go.uber.org/mock/gomock"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/mock"
)

func TestQueue(t *testing.T) {
	controller := gomock.NewController(t)
	defer controller.Finish()

	runners := []*core.Runner{
		{Name: "runner-3", Owner: "org", Arch: core.ArchAmd64, CPU: 2, RAM: 2048},
		{Name: "runner-2", Owner: "org", Arch: core.ArchAmd64, CPU: 2, RAM: 2048},
		{Name: "runner-1", Owner: "org", Arch: core.ArchAmd64, CPU: 2, RAM: 2048},
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	store := mock.NewMockRunnerStore(controller)
	store.EXPECT().ListStatus(ctx, core.RunnerStatusPending).Return(runners, nil).Times(1)
	store.EXPECT().ListStatus(ctx, core.RunnerStatusPending).Return(runners[1:], nil).Times(1)
	store.EXPECT().ListStatus(ctx, core.RunnerStatusPending).Return(runners[2:], nil).Times(1)

	q := newQueue(ctx, store)
	machine := &core.Machine{
		Name:         "machine-1",
		Owner:        "org",
		Arch:         core.ArchAmd64,
		CPU:          4,
		RAMAvailable: 4096,
		RAMLimit:     4096,
		Status:       core.MachineStatusOnline,
	}

	for _, runner := range runners {
		next, err := q.Request(ctx, machine)
		if err != nil {
			t.Error(err)
			return
		}
		if got, want := next, runner; got != want {
			t.Errorf("Want runner %s, got %s", want.Name, got.Name)
		}
	}
}

func TestQueueCancel(t *testing.T) {
	controller := gomock.NewController(t)
	defer controller.Finish()

	ctx, cancel := context.WithCancel(context.Background())
	store := mock.NewMockRunnerStore(controller)
	store.EXPECT().ListStatus(ctx, core.RunnerStatusPending).Return(nil, nil)

	q := newQueue(ctx, store)

	var wg sync.WaitGroup
	wg.Add(1)

	go func() {
		machine := &core.Machine{
			Name:         "machine-1",
			Owner:        "org",
			Arch:         core.ArchAmd64,
			CPU:          4,
			RAMAvailable: 4096,
			RAMLimit:     4096,
			Status:       core.MachineStatusOnline,
		}

		runner, err := q.Request(ctx, machine)
		if err != context.Canceled {
			t.Errorf("Expected context.Canceled error, got %s", err)
		}
		if runner != nil {
			t.Errorf("Expect nil runner when subscribe canceled")
		}
		wg.Done()
	}()
	<-time.After(10 * time.Millisecond)

	q.Lock()
	count := len(q.workers)
	q.Unlock()

	if got, want := count, 1; got != want {
		t.Errorf("Want %d listener, got %d", want, got)
	}

	cancel()
	wg.Wait()
}

func TestQueueDeadlock(t *testing.T) {
	controller := gomock.NewController(t)
	defer controller.Finish()

	n := 10
	donechan := make(chan struct{}, n)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	store := mock.NewMockRunnerStore(controller)
	store.EXPECT().ListStatus(ctx, core.RunnerStatusPending).Return(incompleteRunners(n), nil).AnyTimes()

	q := newQueue(ctx, store)
	machine := &core.Machine{
		Name:         "machine-1",
		Owner:        "org",
		Arch:         core.ArchAmd64,
		CPU:          4,
		RAMAvailable: 4096,
		RAMLimit:     4096,
		Status:       core.MachineStatusOnline,
	}
	doWork := func(i int) bool {
		select {
		case <-ctx.Done():
			return false
		default:
		}
		ctx, cancel := context.WithTimeout(ctx,
			time.Duration(i+rand.Intn(1000/n))*time.Millisecond)
		defer cancel()
		if i%3 == 0 {
			// Randomly cancel some contexts to simulate timeouts
			cancel()
		}
		_, err := q.Request(ctx, machine)
		if err != nil && err != context.Canceled && err !=
			context.DeadlineExceeded {
			t.Errorf("Expected context.Canceled or context.DeadlineExceeded error, got %s", err)
		}
		select {
		case donechan <- struct{}{}:
		case <-ctx.Done():
		}
		return true
	}
	for i := 0; i < n; i++ {
		go func(i int) {
			// Spawn n workers, doing work until the parent context is canceled
			for doWork(i) {
			}
		}(i)
	}
	// Wait for n * 10 tasks to complete, then exit and cancel all the workers.
	for seen := 0; seen < n*10; seen++ {
		<-donechan
	}
}

func incompleteRunners(n int) []*core.Runner {
	ret := make([]*core.Runner, n)
	for i := range ret {
		ret[i] = &core.Runner{Owner: "org", Arch: core.ArchAmd64, CPU: 2, RAM: 2048}
	}
	return ret
}
