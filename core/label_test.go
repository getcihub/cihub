package core

import "testing"

func TestLabel_Validate_Success(t *testing.T) {
	tests := []struct {
		name  string
		label *Label
	}{
		{
			name: "valid 2vcpu amd64 ubuntu2404",
			label: &Label{
				ID:     "cihub-2vcpu-amd64-ubuntu2404",
				Arch:   "amd64",
				OS:  "ubuntu2404",
				Memory: 2048,
				VCPU:   2,
			},
		},
		{
			name: "valid 4vcpu arm64 ubuntu2204",
			label: &Label{
				ID:     "cihub-4vcpu-arm64-ubuntu2204",
				Arch:   "arm64",
				OS:  "ubuntu2204",
				Memory: 4096,
				VCPU:   4,
			},
		},
		{
			name: "valid 8vcpu amd64 ubuntu2204",
			label: &Label{
				ID:     "cihub-8vcpu-amd64-ubuntu2204",
				Arch:   "amd64",
				OS:  "ubuntu2204",
				Memory: 8192,
				VCPU:   8,
			},
		},
		{
			name: "valid minimal 1vcpu arm64 ubuntu2404",
			label: &Label{
				ID:     "cihub-1vcpu-arm64-ubuntu2404",
				Arch:   "arm64",
				OS:  "ubuntu2404",
				Memory: 1024,
				VCPU:   1,
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.label.Validate()
			if err != nil {
				t.Errorf("Validate() should not return error, got %v", err)
			}
		})
	}
}

func TestLabel_Validate_InvalidPrefix(t *testing.T) {
	tests := []struct {
		name  string
		label *Label
	}{
		{
			name: "no prefix",
			label: &Label{
				ID:     "2vcpu-amd64-ubuntu2404",
				Arch:   "amd64",
				OS:  "ubuntu2404",
				Memory: 2048,
				VCPU:   2,
			},
		},
		{
			name: "wrong prefix",
			label: &Label{
				ID:     "runner-2vcpu-amd64-ubuntu2404",
				Arch:   "amd64",
				OS:  "ubuntu2404",
				Memory: 2048,
				VCPU:   2,
			},
		},
		{
			name: "empty id",
			label: &Label{
				ID:     "",
				Arch:   "amd64",
				OS:  "ubuntu2404",
				Memory: 2048,
				VCPU:   2,
			},
		},
		{
			name: "uppercase prefix",
			label: &Label{
				ID:     "CIHUB-2vcpu-amd64-ubuntu2404",
				Arch:   "amd64",
				OS:  "ubuntu2404",
				Memory: 2048,
				VCPU:   2,
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.label.Validate()
			if err == nil {
				t.Error("Validate() should return error for invalid prefix")
			}
			if err.Error() != "invalid label ID prefix: "+tt.label.ID {
				t.Errorf("Validate() returned unexpected error message: %v", err)
			}
		})
	}
}

func TestLabel_Validate_InvalidArch(t *testing.T) {
	tests := []struct {
		name string
		arch string
	}{
		{
			name: "invalid arch x86_64",
			arch: "x86_64",
		},
		{
			name: "invalid arch x64",
			arch: "x64",
		},
		{
			name: "invalid arch arm",
			arch: "arm",
		},
		{
			name: "invalid arch arm64e",
			arch: "arm64e",
		},
		{
			name: "empty arch",
			arch: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			label := &Label{
				ID:     "cihub-2vcpu-amd64-ubuntu2404",
				Arch:   tt.arch,
				OS:  "ubuntu2404",
				Memory: 2048,
				VCPU:   2,
			}
			err := label.Validate()
			if err == nil {
				t.Error("Validate() should return error for invalid architecture")
			}
			if err.Error() != "invalid architecture: "+tt.arch+" (must be amd64 or arm64)" {
				t.Errorf("Validate() returned unexpected error message: %v", err)
			}
		})
	}
}

func TestLabel_Validate_InvalidVCPU(t *testing.T) {
	tests := []struct {
		name string
		vcpu int64
	}{
		{
			name: "zero vcpu",
			vcpu: 0,
		},
		{
			name: "negative vcpu",
			vcpu: -1,
		},
		{
			name: "large negative vcpu",
			vcpu: -9999,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			label := &Label{
				ID:     "cihub-2vcpu-amd64-ubuntu2404",
				Arch:   "amd64",
				OS:  "ubuntu2404",
				Memory: 2048,
				VCPU:   tt.vcpu,
			}
			err := label.Validate()
			if err == nil {
				t.Error("Validate() should return error for invalid VCPU")
			}
			t.Logf("Validate() error: %v", err)
		})
	}
}

func TestLabel_Validate_InvalidMemory(t *testing.T) {
	tests := []struct {
		name   string
		memory int64
	}{
		{
			name:   "zero memory",
			memory: 0,
		},
		{
			name:   "negative memory",
			memory: -1,
		},
		{
			name:   "large negative memory",
			memory: -8192,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			label := &Label{
				ID:     "cihub-2vcpu-amd64-ubuntu2404",
				Arch:   "amd64",
				OS:  "ubuntu2404",
				Memory: tt.memory,
				VCPU:   2,
			}
			err := label.Validate()
			if err == nil {
				t.Error("Validate() should return error for invalid Memory")
			}
			t.Logf("Validate() error: %v", err)
		})
	}
}

func TestLabel_Validate_MultipleErrors(t *testing.T) {
	// When multiple validation rules fail, the first one checked is returned
	label := &Label{
		ID:     "invalid",      // No cihub- prefix
		Arch:   "x86",          // Invalid arch
		OS:     "debian",       // Invalid OS
		Memory: 0,              // Invalid memory
		VCPU:   0,              // Invalid VCPU
	}
	err := label.Validate()
	if err == nil {
		t.Error("Validate() should return error")
	}
	// ID validation is checked first
	if err.Error() != "invalid label ID prefix: invalid" {
		t.Errorf("Validate() should check ID prefix first, got: %v", err)
	}
}

func TestLabels_Has_Match(t *testing.T) {
	labels := Labels{
		"cihub-2vcpu-amd64-ubuntu2404": {
			ID:     "cihub-2vcpu-amd64-ubuntu2404",
			Arch:   "amd64",
			OS:  "ubuntu2404",
			Memory: 2048,
			VCPU:   2,
		},
		"cihub-4vcpu-arm64-ubuntu2204": {
			ID:     "cihub-4vcpu-arm64-ubuntu2204",
			Arch:   "arm64",
			OS:  "ubuntu2204",
			Memory: 4096,
			VCPU:   4,
		},
	}

	tests := []struct {
		name      string
		requested []string
		expected  bool
	}{
		{
			name:      "single matching label",
			requested: []string{"cihub-2vcpu-amd64-ubuntu2404"},
			expected:  true,
		},
		{
			name:      "matching label with others",
			requested: []string{"self-hosted", "docker", "cihub-2vcpu-amd64-ubuntu2404"},
			expected:  true,
		},
		{
			name:      "first label matches",
			requested: []string{"cihub-4vcpu-arm64-ubuntu2204", "self-hosted"},
			expected:  true,
		},
		{
			name:      "multiple configured labels requested",
			requested: []string{"cihub-2vcpu-amd64-ubuntu2404", "cihub-4vcpu-arm64-ubuntu2204"},
			expected:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := labels.Has(tt.requested)
			if result != tt.expected {
				t.Errorf("Has(%v) = %v, expected %v", tt.requested, result, tt.expected)
			}
		})
	}
}

func TestLabels_Has_NoMatch(t *testing.T) {
	labels := Labels{
		"cihub-2vcpu-amd64-ubuntu2404": {
			ID:     "cihub-2vcpu-amd64-ubuntu2404",
			Arch:   "amd64",
			OS:  "ubuntu2404",
			Memory: 2048,
			VCPU:   2,
		},
	}

	tests := []struct {
		name      string
		requested []string
	}{
		{
			name:      "no matching labels",
			requested: []string{"self-hosted", "docker"},
		},
		{
			name:      "empty requested labels",
			requested: []string{},
		},
		{
			name:      "wrong cihub label",
			requested: []string{"cihub-4vcpu-arm64-ubuntu2204", "cihub-windows"},
		},
		{
			name:      "case mismatch",
			requested: []string{"CIHUB-2VCPU-AMD64-UBUNTU2404"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := labels.Has(tt.requested)
			if result != false {
				t.Errorf("Has(%v) = %v, expected false", tt.requested, result)
			}
		})
	}
}

func TestLabels_Has_EmptyLabels(t *testing.T) {
	labels := Labels{}

	requested := []string{"cihub-2vcpu-amd64-ubuntu2404", "self-hosted"}
	result := labels.Has(requested)
	if result != false {
		t.Errorf("Has(%v) on empty Labels = %v, expected false", requested, result)
	}
}

func TestLabels_Has_NilSlice(t *testing.T) {
	labels := Labels{
		"cihub-2vcpu-amd64-ubuntu2404": {
			ID:   "cihub-2vcpu-amd64-ubuntu2404",
			Arch: "amd64",
			OS:   "ubuntu2404",
		},
	}

	// Has should handle nil slice gracefully
	result := labels.Has(nil)
	if result != false {
		t.Errorf("Has(nil) = %v, expected false", result)
	}
}
