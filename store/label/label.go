package label

import (
	"context"
	"errors"
	"fmt"

	"github.com/getcihub/cihub/core"
)

// ErrLabelNotFound is returned when a label cannot be found.
var ErrLabelNotFound = errors.New("label not found")

type store struct {
	labels map[string]*core.Label
}

// New returns a new in-memory LabelStore populated from configuration.
// Labels are immutable after initialization and loaded from config at startup.
func New(labels []*core.Label) (core.LabelStore, error) {
	m := make(map[string]*core.Label, len(labels))
	for _, label := range labels {
		_, ok := m[label.Name]
		if ok {
			return nil, fmt.Errorf("label %s is already defined", label.Name)
		}
		m[label.Name] = label
	}

	return &store{m}, nil
}

// Find returns a label by its name.
func (s *store) Find(ctx context.Context, name string) (*core.Label, error) {
	label, ok := s.labels[name]
	if !ok {
		return nil, ErrLabelNotFound
	}
	return label, nil
}

// List returns all available labels.
func (s *store) List(ctx context.Context) ([]*core.Label, error) {
	labels := make([]*core.Label, 0, len(s.labels))
	for _, label := range s.labels {
		labels = append(labels, label)
	}
	return labels, nil
}
