package installation

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db/dbtest"
)

func TestCreate(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	tests := []struct {
		name  string
		input *core.Installation
		want  *core.Installation
	}{
		{
			name: "create organization installation",
			input: &core.Installation{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
			},
			want: &core.Installation{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
			},
		},
		{
			name: "create user installation",
			input: &core.Installation{
				ID:        54321,
				Login:     "octocat",
				Avatar:    "https://avatars.githubusercontent.com/u/54321",
				Type:      core.InstallationTypeUser,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
			},
			want: &core.Installation{
				ID:        54321,
				Login:     "octocat",
				Avatar:    "https://avatars.githubusercontent.com/u/54321",
				Type:      core.InstallationTypeUser,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
			},
		},
		{
			name: "create suspended installation",
			input: &core.Installation{
				ID:        99999,
				Login:     "suspended-org",
				Avatar:    "https://avatars.githubusercontent.com/u/99999",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 1609545600,
				Updated:   1609545600,
			},
			want: &core.Installation{
				ID:        99999,
				Login:     "suspended-org",
				Avatar:    "https://avatars.githubusercontent.com/u/99999",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 1609545600,
				Updated:   1609545600,
			},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			err := store.Create(ctx, test.input)
			require.NoError(t, err)
			assert.Equal(t, test.want, test.input)
		})
	}
}

func TestFind(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Setup: Create test data
	installation := &core.Installation{
		ID:        12345,
		Login:     "acme",
		Avatar:    "https://avatars.githubusercontent.com/u/12345",
		Type:      core.InstallationTypeOrganization,
		Created:   1609459200,
		Suspended: 0,
		Updated:   1609459200,
	}
	createErr := store.Create(ctx, installation)
	require.NoError(t, createErr)

	tests := []struct {
		name        string
		id          int64
		wantErr     bool
		wantFound   bool
		wantInstall *core.Installation
	}{
		{
			name:      "find existing installation",
			id:        12345,
			wantErr:   false,
			wantFound: true,
			wantInstall: &core.Installation{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
			},
		},
		{
			name:      "find non-existent installation",
			id:        99999,
			wantErr:   true,
			wantFound: false,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			got, err := store.Find(ctx, test.id)
			if test.wantErr {
				assert.Error(t, err)
			} else {
				require.NoError(t, err)
				assert.Equal(t, test.wantInstall, got)
			}
		})
	}
}

func TestFindOwner(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Setup: Create test data
	installation := &core.Installation{
		ID:        12345,
		Login:     "acme",
		Avatar:    "https://avatars.githubusercontent.com/u/12345",
		Type:      core.InstallationTypeOrganization,
		Created:   1609459200,
		Suspended: 0,
		Updated:   1609459200,
	}
	createErr := store.Create(ctx, installation)
	require.NoError(t, createErr)

	tests := []struct {
		name        string
		owner       string
		wantErr     bool
		wantFound   bool
		wantInstall *core.Installation
	}{
		{
			name:      "find by existing owner",
			owner:     "acme",
			wantErr:   false,
			wantFound: true,
			wantInstall: &core.Installation{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
			},
		},
		{
			name:      "find by non-existent owner",
			owner:     "nonexistent",
			wantErr:   true,
			wantFound: false,
		},
		{
			name:      "case insensitive search",
			owner:     "ACME",
			wantErr:   false,
			wantFound: true,
			wantInstall: &core.Installation{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
			},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			got, err := store.FindLogin(ctx, test.owner)
			if test.wantErr {
				assert.Error(t, err)
			} else {
				require.NoError(t, err)
				assert.Equal(t, test.wantInstall, got)
			}
		})
	}
}

func TestUpdate(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Setup: Create test data
	installation := &core.Installation{
		ID:        12345,
		Login:     "acme",
		Avatar:    "https://avatars.githubusercontent.com/u/12345",
		Type:      core.InstallationTypeOrganization,
		Created:   1609459200,
		Suspended: 0,
		Updated:   1609459200,
	}
	createErr := store.Create(ctx, installation)
	require.NoError(t, createErr)

	tests := []struct {
		name    string
		input   *core.Installation
		wantErr bool
	}{
		{
			name: "update avatar",
			input: &core.Installation{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345?v=4",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609545600,
			},
			wantErr: false,
		},
		{
			name: "update suspension status",
			input: &core.Installation{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345?v=4",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 1609545600,
				Updated:   1609545600,
			},
			wantErr: false,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			err := store.Update(ctx, test.input)
			if test.wantErr {
				assert.Error(t, err)
			} else {
				require.NoError(t, err)

				// Verify the update
				got, err := store.Find(ctx, test.input.ID)
				require.NoError(t, err)
				assert.Equal(t, test.input, got)
			}
		})
	}
}

func TestDelete(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Setup: Create test data
	installation := &core.Installation{
		ID:        12345,
		Login:     "acme",
		Avatar:    "https://avatars.githubusercontent.com/u/12345",
		Type:      core.InstallationTypeOrganization,
		Created:   1609459200,
		Suspended: 0,
		Updated:   1609459200,
	}
	createErr := store.Create(ctx, installation)
	require.NoError(t, createErr)

	tests := []struct {
		name    string
		id      int64
		wantErr bool
	}{
		{
			name:    "delete existing installation",
			id:      12345,
			wantErr: false,
		},
		{
			name:    "delete non-existent installation",
			id:      99999,
			wantErr: false, // DELETE is idempotent
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			installation := &core.Installation{ID: test.id}
			err := store.Delete(ctx, installation)
			if test.wantErr {
				assert.Error(t, err)
			} else {
				require.NoError(t, err)

				// For existing installation, verify it's deleted
				if test.id == 12345 {
					_, err := store.Find(ctx, test.id)
					assert.Error(t, err)
				}
			}
		})
	}
}

func TestList(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// We need a membership store to test List properly
	// For now, create the user and installations directly

	user := &core.User{
		ID:    1,
		Login: "testuser",
		Email: "test@example.com",
	}

	// Create installations
	installations := []*core.Installation{
		{
			ID:        12345,
			Login:     "acme",
			Avatar:    "https://avatars.githubusercontent.com/u/12345",
			Type:      core.InstallationTypeOrganization,
			Created:   1609459200,
			Suspended: 0,
			Updated:   1609459200,
		},
		{
			ID:        54321,
			Login:     "octocat",
			Avatar:    "https://avatars.githubusercontent.com/u/54321",
			Type:      core.InstallationTypeUser,
			Created:   1609459200,
			Suspended: 0,
			Updated:   1609459200,
		},
	}

	for _, inst := range installations {
		err := store.Create(ctx, inst)
		require.NoError(t, err)
	}

	// List installations for user (should return empty slice since no memberships exist)
	got, err := store.List(ctx, user)
	require.NoError(t, err)
	assert.Equal(t, []*core.Installation{}, got)
}
