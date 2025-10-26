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

	jobs := []*core.Job{
		{ID: 3, Labels: []string{"self-hosted", "linux", "amd64"}, Arch: "amd64", OS: "ubuntu2404", Memory: 2048, VCPU: 2},
		{ID: 2, Labels: []string{"self-hosted", "linux", "amd64"}, Arch: "amd64", OS: "ubuntu2404", Memory: 2048, VCPU: 2},
		{ID: 1, Labels: []string{"self-hosted", "linux", "amd64"}, Arch: "amd64", OS: "ubuntu2404", Memory: 2048, VCPU: 2},
	}

	ctx := context.Background()
	store := mock.NewMockJobStore(controller)
	store.EXPECT().ListStatus(ctx, core.JobStatusQueued).Return(jobs, nil).Times(1)
	store.EXPECT().ListStatus(ctx, core.JobStatusQueued).Return(jobs[1:], nil).Times(1)
	store.EXPECT().ListStatus(ctx, core.JobStatusQueued).Return(jobs[2:], nil).Times(1)

	q := newQueue(ctx, store)
	for _, job := range jobs {
		next, err := q.Request(ctx, &core.Filter{Arch: "amd64", Memory: 2048, VCPU: 2})
		if err != nil {
			t.Error(err)
			return
		}
		if got, want := next, job; got != want {
			t.Errorf("Want job %d, got %d", want.ID, got.ID)
		}
	}
}

func TestQueueCancel(t *testing.T) {
	controller := gomock.NewController(t)
	defer controller.Finish()

	ctx, cancel := context.WithCancel(context.Background())
	store := mock.NewMockJobStore(controller)
	store.EXPECT().ListStatus(ctx, core.JobStatusQueued).Return(nil, nil)

	q := newQueue(ctx, store)

	var wg sync.WaitGroup
	wg.Add(1)

	go func() {
		job, err := q.Request(ctx, &core.Filter{Arch: "amd64", Memory: 2048, VCPU: 2})
		if err != context.Canceled {
			t.Errorf("Expected context.Canceled error, got %s", err)
		}
		if job != nil {
			t.Errorf("Expect nil job when subscribe canceled")
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
	store := mock.NewMockJobStore(controller)
	store.EXPECT().ListStatus(ctx, core.JobStatusQueued).Return(incomplete(n)).AnyTimes()

	q := newQueue(ctx, store)
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
		_, err := q.Request(ctx, &core.Filter{Arch: "amd64", Memory: 2048, VCPU: 2})
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

func incomplete(n int) ([]*core.Job, error) {
	ret := make([]*core.Job, n)
	for i := range ret {
		ret[i] = &core.Job{Labels: []string{"self-hosted", "linux", "amd64"}, Arch: "amd64", OS: "ubuntu2404", Memory: 2048, VCPU: 2}
	}
	return ret, nil
}
