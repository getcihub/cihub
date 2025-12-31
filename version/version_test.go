package version

import "testing"

func TestVersion(t *testing.T) {
	if got, want := Version.String(), "0.2.0"; got != want {
		t.Errorf("Want version %s, got %s", want, got)
	}
}
