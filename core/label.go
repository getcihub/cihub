package core

import "context"

// Label defines resource specifications for runners.
// Labels are used in GitHub Actions workflows via "runs-on" to specify runner requirements.
// All label names must start with "cihub" and be unique within the system.
type Label struct {
	Name    string `json:"name"`
	CPU     int    `json:"cpu"`
	RAM     int    `json:"ram"`
	Storage int    `json:"storage"`
	Kernel  string `json:"kernel"`
	Ubuntu  string `json:"ubuntu"`
}

// LabelStore defines operations for working with runner labels.
type LabelStore interface {
	// Find returns a label by its name.
	Find(ctx context.Context, name string) (*Label, error)

	// List returns all available labels.
	List(ctx context.Context) ([]*Label, error)
}
