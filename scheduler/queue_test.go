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
		{ID: 3, Labels: []string{"self-hosted", "linux", "amd64"}},
		{ID: 2, Labels: []string{"self-hosted", "linux", "amd64"}},
		{ID: 1, Labels: []string{"self-hosted", "linux", "amd64"}},
	}

	ctx := context.Background()
	store := mock.NewMockJobStore(controller)
	store.EXPECT().ListStatus(ctx, core.JobStatusQueued).Return(jobs, nil).Times(1)
	store.EXPECT().ListStatus(ctx, core.JobStatusQueued).Return(jobs[1:], nil).Times(1)
	store.EXPECT().ListStatus(ctx, core.JobStatusQueued).Return(jobs[2:], nil).Times(1)

	q := newQueue(ctx, store)
	for _, job := range jobs {
		next, err := q.Request(ctx, []string{"self-hosted", "linux", "amd64"})
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
		job, err := q.Request(ctx, []string{"self-hosted", "linux", "amd64"})
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

func TestCheckLabels(t *testing.T) {
	tests := []struct {
		name string
		a    []string
		b    []string
		want bool
	}{
		{
			name: "equal slices same order",
			a:    []string{"linux", "amd64", "docker"},
			b:    []string{"linux", "amd64", "docker"},
			want: true,
		},
		{
			name: "equal slices different order",
			a:    []string{"linux", "amd64", "docker"},
			b:    []string{"docker", "linux", "amd64"},
			want: true,
		},
		{
			name: "different length slices",
			a:    []string{"linux", "amd64"},
			b:    []string{"linux", "amd64", "docker"},
			want: false,
		},
		{
			name: "both empty slices",
			a:    []string{},
			b:    []string{},
			want: true,
		},
		{
			name: "one empty one non-empty",
			a:    []string{"linux"},
			b:    []string{},
			want: false,
		},
		{
			name: "both nil slices",
			a:    nil,
			b:    nil,
			want: true,
		},
		{
			name: "one nil one empty",
			a:    nil,
			b:    []string{},
			want: true,
		},
		{
			name: "different elements same length",
			a:    []string{"linux", "amd64"},
			b:    []string{"darwin", "arm64"},
			want: false,
		},
		{
			name: "partially overlapping",
			a:    []string{"linux", "amd64"},
			b:    []string{"linux", "arm64"},
			want: false,
		},
		{
			name: "single element match",
			a:    []string{"linux"},
			b:    []string{"linux"},
			want: true,
		},
		{
			name: "single element mismatch",
			a:    []string{"linux"},
			b:    []string{"darwin"},
			want: false,
		},
		{
			name: "duplicate elements in first slice",
			a:    []string{"linux", "linux", "amd64"},
			b:    []string{"linux", "amd64"},
			want: false,
		},
		{
			name: "duplicate elements in both slices same count",
			a:    []string{"linux", "linux", "amd64"},
			b:    []string{"linux", "linux", "amd64"},
			want: true,
		},
		{
			name: "duplicate elements in both slices different order",
			a:    []string{"linux", "amd64", "linux"},
			b:    []string{"amd64", "linux", "linux"},
			want: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := checkLabels(tt.a, tt.b)
			if got != tt.want {
				t.Errorf("checkLabels(%v, %v) = %v, want %v", tt.a, tt.b, got, tt.want)
			}
		})
	}
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
		_, err := q.Request(ctx, []string{"self-hosted", "linux", "amd64"})
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
		ret[i] = &core.Job{Labels: []string{"self-hosted", "linux", "amd64"}}
	}
	return ret, nil
}
