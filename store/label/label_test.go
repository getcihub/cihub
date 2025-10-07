package label

import (
	"context"
	"errors"
	"testing"

	"github.com/getcihub/cihub/core"
)

var noContext = context.Background()

func TestNew(t *testing.T) {
	tests := []struct {
		name    string
		labels  []*core.Label
		wantErr bool
	}{
		{
			name:    "empty labels",
			labels:  []*core.Label{},
			wantErr: false,
		},
		{
			name: "single label",
			labels: []*core.Label{
				{
					Name:    "cihub-small",
					CPU:     2,
					RAM:     2048,
					Storage: 10,
					Kernel:  "vmlinux-5.10",
					Ubuntu:  "ubuntu22.04",
				},
			},
			wantErr: false,
		},
		{
			name: "multiple labels",
			labels: []*core.Label{
				{Name: "cihub-small", CPU: 2, RAM: 2048, Storage: 10, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
				{Name: "cihub-medium", CPU: 4, RAM: 4096, Storage: 20, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
				{Name: "cihub-large", CPU: 8, RAM: 8192, Storage: 50, Kernel: "vmlinux-6.1", Ubuntu: "ubuntu24.04"},
			},
			wantErr: false,
		},
		{
			name: "duplicate label names",
			labels: []*core.Label{
				{Name: "cihub-small", CPU: 2, RAM: 2048, Storage: 10, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
				{Name: "cihub-small", CPU: 4, RAM: 4096, Storage: 20, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			store, err := New(tt.labels)
			if (err != nil) != tt.wantErr {
				t.Errorf("New() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr && store == nil {
				t.Error("New() returned nil store")
			}
		})
	}
}

func TestStore_Find(t *testing.T) {
	labels := []*core.Label{
		{Name: "cihub-small", CPU: 2, RAM: 2048, Storage: 10, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
		{Name: "cihub-medium", CPU: 4, RAM: 4096, Storage: 20, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
	}

	store, err := New(labels)
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}

	tests := []struct {
		name      string
		labelName string
		want      *core.Label
		wantErr   error
	}{
		{
			name:      "find existing label",
			labelName: "cihub-small",
			want:      labels[0],
			wantErr:   nil,
		},
		{
			name:      "find another existing label",
			labelName: "cihub-medium",
			want:      labels[1],
			wantErr:   nil,
		},
		{
			name:      "label not found",
			labelName: "cihub-nonexistent",
			want:      nil,
			wantErr:   ErrLabelNotFound,
		},
		{
			name:      "empty name",
			labelName: "",
			want:      nil,
			wantErr:   ErrLabelNotFound,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := store.Find(noContext, tt.labelName)
			if !errors.Is(err, tt.wantErr) {
				t.Errorf("Find() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if tt.want != nil && got != tt.want {
				t.Errorf("Find() got = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestStore_List(t *testing.T) {
	tests := []struct {
		name      string
		labels    []*core.Label
		wantCount int
	}{
		{
			name:      "empty store",
			labels:    []*core.Label{},
			wantCount: 0,
		},
		{
			name: "single label",
			labels: []*core.Label{
				{Name: "cihub-small", CPU: 2, RAM: 2048, Storage: 10, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
			},
			wantCount: 1,
		},
		{
			name: "multiple labels",
			labels: []*core.Label{
				{Name: "cihub-small", CPU: 2, RAM: 2048, Storage: 10, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
				{Name: "cihub-medium", CPU: 4, RAM: 4096, Storage: 20, Kernel: "vmlinux-5.10", Ubuntu: "ubuntu22.04"},
				{Name: "cihub-large", CPU: 8, RAM: 8192, Storage: 50, Kernel: "vmlinux-6.1", Ubuntu: "ubuntu24.04"},
			},
			wantCount: 3,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			store, err := New(tt.labels)
			if err != nil {
				t.Fatalf("New() failed: %v", err)
			}

			got, err := store.List(noContext)
			if err != nil {
				t.Errorf("List() error = %v", err)
				return
			}
			if len(got) != tt.wantCount {
				t.Errorf("List() returned %d labels, want %d", len(got), tt.wantCount)
			}

			// Verify all labels are present
			labelMap := make(map[string]bool)
			for _, label := range got {
				labelMap[label.Name] = true
			}
			for _, expected := range tt.labels {
				if !labelMap[expected.Name] {
					t.Errorf("List() missing label %s", expected.Name)
				}
			}
		})
	}
}
