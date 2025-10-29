package label

import (
	"testing"
)

func TestParse(t *testing.T) {
	tests := []struct {
		name      string
		labelStr  string
		wantError bool
		wantLabel *Label
	}{
		{
			name:      "valid 2cpu 4gb amd64",
			labelStr:  "cihub-2cpu-4gb",
			wantError: false,
			wantLabel: &Label{CPU: 2, RAM: 4096, Arch: "amd64"},
		},
		{
			name:      "valid 4cpu 8gb arm64",
			labelStr:  "cihub-4cpu-8gb-arm64",
			wantError: false,
			wantLabel: &Label{CPU: 4, RAM: 8192, Arch: "arm64"},
		},
		{
			name:      "valid 2cpu 2048mb amd64",
			labelStr:  "cihub-2cpu-2048mb",
			wantError: false,
			wantLabel: &Label{CPU: 2, RAM: 2048, Arch: "amd64"},
		},
		{
			name:      "valid 2cpu 2048mb with explicit amd64",
			labelStr:  "cihub-2cpu-2048mb-amd64",
			wantError: false,
			wantLabel: &Label{CPU: 2, RAM: 2048, Arch: "amd64"},
		},
		{
			name:      "not a cihub label",
			labelStr:  "self-hosted",
			wantError: false,
			wantLabel: nil,
		},
		{
			name:      "invalid cihub format missing cpu",
			labelStr:  "cihub-4gb",
			wantError: true,
			wantLabel: nil,
		},
		{
			name:      "invalid cihub format missing ram unit",
			labelStr:  "cihub-2cpu-4",
			wantError: true,
			wantLabel: nil,
		},
		{
			name:      "invalid cihub format invalid unit",
			labelStr:  "cihub-2cpu-4tb",
			wantError: true,
			wantLabel: nil,
		},
		{
			name:      "invalid cihub format invalid arch",
			labelStr:  "cihub-2cpu-4gb-x86",
			wantError: true,
			wantLabel: nil,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			lbl, err := Parse(tt.labelStr)

			if (err != nil) != tt.wantError {
				t.Errorf("Parse(%q) error = %v, wantError %v", tt.labelStr, err, tt.wantError)
				return
			}

			if tt.wantLabel == nil {
				if lbl != nil {
					t.Errorf("Parse(%q) = %v, want nil", tt.labelStr, lbl)
				}
				return
			}

			if lbl == nil {
				t.Errorf("Parse(%q) = nil, want %v", tt.labelStr, tt.wantLabel)
				return
			}

			if lbl.CPU != tt.wantLabel.CPU {
				t.Errorf("Parse(%q) CPU = %d, want %d", tt.labelStr, lbl.CPU, tt.wantLabel.CPU)
			}
			if lbl.RAM != tt.wantLabel.RAM {
				t.Errorf("Parse(%q) RAM = %d, want %d", tt.labelStr, lbl.RAM, tt.wantLabel.RAM)
			}
			if lbl.Arch != tt.wantLabel.Arch {
				t.Errorf("Parse(%q) Arch = %s, want %s", tt.labelStr, lbl.Arch, tt.wantLabel.Arch)
			}
		})
	}
}

func TestResolve(t *testing.T) {
	tests := []struct {
		name      string
		labels    []string
		wantError bool
		wantLabel *Label
	}{
		{
			name:      "first label is cihub label",
			labels:    []string{"cihub-2cpu-4gb", "self-hosted"},
			wantError: false,
			wantLabel: &Label{CPU: 2, RAM: 4096, Arch: "amd64"},
		},
		{
			name:      "cihub label in middle",
			labels:    []string{"self-hosted", "cihub-2cpu-4gb", "linux"},
			wantError: false,
			wantLabel: &Label{CPU: 2, RAM: 4096, Arch: "amd64"},
		},
		{
			name:      "multiple cihub labels uses first",
			labels:    []string{"cihub-4cpu-8gb-arm64", "cihub-2cpu-4gb"},
			wantError: false,
			wantLabel: &Label{CPU: 4, RAM: 8192, Arch: "arm64"},
		},
		{
			name:      "no cihub labels",
			labels:    []string{"self-hosted", "linux"},
			wantError: false,
			wantLabel: nil,
		},
		{
			name:      "invalid cihub label",
			labels:    []string{"self-hosted", "cihub-invalid"},
			wantError: true,
			wantLabel: nil,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			lbl, err := Resolve(tt.labels)

			if (err != nil) != tt.wantError {
				t.Errorf("Resolve(%v) error = %v, wantError %v", tt.labels, err, tt.wantError)
				return
			}

			if tt.wantLabel == nil {
				if lbl != nil {
					t.Errorf("Resolve(%v) = %v, want nil", tt.labels, lbl)
				}
				return
			}

			if lbl == nil {
				t.Errorf("Resolve(%v) = nil, want %v", tt.labels, tt.wantLabel)
				return
			}

			if lbl.CPU != tt.wantLabel.CPU {
				t.Errorf("Resolve(%v) CPU = %d, want %d", tt.labels, lbl.CPU, tt.wantLabel.CPU)
			}
			if lbl.RAM != tt.wantLabel.RAM {
				t.Errorf("Resolve(%v) RAM = %d, want %d", tt.labels, lbl.RAM, tt.wantLabel.RAM)
			}
			if lbl.Arch != tt.wantLabel.Arch {
				t.Errorf("Resolve(%v) Arch = %s, want %s", tt.labels, lbl.Arch, tt.wantLabel.Arch)
			}
		})
	}
}

func TestHas(t *testing.T) {
	tests := []struct {
		name   string
		labels []string
		want   bool
	}{
		{
			name:   "has cihub label",
			labels: []string{"cihub-2cpu-4gb"},
			want:   true,
		},
		{
			name:   "has cihub label among others",
			labels: []string{"self-hosted", "cihub-2cpu-4gb", "linux"},
			want:   true,
		},
		{
			name:   "no cihub labels",
			labels: []string{"self-hosted", "linux"},
			want:   false,
		},
		{
			name:   "empty labels",
			labels: []string{},
			want:   false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := Has(tt.labels)
			if got != tt.want {
				t.Errorf("Has(%v) = %v, want %v", tt.labels, got, tt.want)
			}
		})
	}
}

func TestValidate(t *testing.T) {
	tests := []struct {
		name      string
		label     *Label
		wantError bool
	}{
		{
			name:      "valid label",
			label:     &Label{CPU: 2, RAM: 2048, Arch: "amd64"},
			wantError: false,
		},
		{
			name:      "valid arm64 label",
			label:     &Label{CPU: 4, RAM: 4096, Arch: "arm64"},
			wantError: false,
		},
		{
			name:      "invalid cpu zero",
			label:     &Label{CPU: 0, RAM: 2048, Arch: "amd64"},
			wantError: true,
		},
		{
			name:      "invalid cpu negative",
			label:     &Label{CPU: -1, RAM: 2048, Arch: "amd64"},
			wantError: true,
		},
		{
			name:      "invalid ram zero",
			label:     &Label{CPU: 2, RAM: 0, Arch: "amd64"},
			wantError: true,
		},
		{
			name:      "invalid ram negative",
			label:     &Label{CPU: 2, RAM: -1024, Arch: "amd64"},
			wantError: true,
		},
		{
			name:      "invalid arch",
			label:     &Label{CPU: 2, RAM: 2048, Arch: "x86"},
			wantError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.label.Validate()
			if (err != nil) != tt.wantError {
				t.Errorf("Label.Validate() error = %v, wantError %v", err, tt.wantError)
			}
		})
	}
}
