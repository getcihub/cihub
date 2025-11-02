package membership

import (
	"context"
	"testing"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
	"github.com/getcihub/cihub/store/shared/db/dbtest"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestFind(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Setup: Create test data
	// First create a user and installation in the database
	err = database.Lock(func(execer db.Execer, binder db.Binder) error {
		// Create a test user
		_, err := execer.Exec(`
			INSERT INTO users (
				user_id, user_login, user_email, user_admin, user_active,
				user_avatar, user_created, user_updated, user_synced, user_syncing,
				user_oauth_token, user_oauth_refresh, user_oauth_expiry, user_token
			) VALUES (1, 'testuser', 'test@example.com', 0, 1, 'avatar.png', 1609459200, 1609459200, 0, 0, 'encrypted_token', 'encrypted_refresh', 0, 'token123')
		`)
		if err != nil {
			return err
		}

		// Create a test installation
		_, err = execer.Exec(`
			INSERT INTO installations (
				installation_id, installation_login, installation_avatar, installation_type,
				installation_created, installation_suspended, installation_updated
			) VALUES (12345, 'acme', 'avatar.png', 'Organization', 1609459200, 0, 1609459200)
		`)
		if err != nil {
			return err
		}

		// Create a test membership
		_, err = execer.Exec(`
			INSERT INTO memberships (
				membership_installation_id, membership_user_id, membership_role, membership_state,
				membership_created, membership_synced, membership_updated
			) VALUES (12345, 1, 'admin', 'active', 1609459200, 1609459200, 1609459200)
		`)
		return err
	})
	require.NoError(t, err)

	tests := []struct {
		name        string
		installID   int64
		userID      int64
		wantErr     bool
		wantMember  *core.Membership
	}{
		{
			name:      "find existing membership",
			installID: 12345,
			userID:    1,
			wantErr:   false,
			wantMember: &core.Membership{
				InstallationID: 12345,
				UserID:         1,
				Role:           "admin",
				State:          "active",
				Created:        1609459200,
				Synced:         1609459200,
				Updated:        1609459200,
			},
		},
		{
			name:      "find non-existent membership",
			installID: 99999,
			userID:    99999,
			wantErr:   true,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			got, err := store.Find(ctx, test.installID, test.userID)
			if test.wantErr {
				assert.Error(t, err)
			} else {
				require.NoError(t, err)
				assert.Equal(t, test.wantMember, got)
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
	err = database.Lock(func(execer db.Execer, binder db.Binder) error {
		// Create a test user
		_, err := execer.Exec(`
			INSERT INTO users (
				user_id, user_login, user_email, user_admin, user_active,
				user_avatar, user_created, user_updated, user_synced, user_syncing,
				user_oauth_token, user_oauth_refresh, user_oauth_expiry, user_token
			) VALUES (2, 'testuser2', 'test2@example.com', 0, 1, 'avatar.png', 1609459200, 1609459200, 0, 0, 'encrypted_token', 'encrypted_refresh', 0, 'token123')
		`)
		if err != nil {
			return err
		}

		// Create a test installation
		_, err = execer.Exec(`
			INSERT INTO installations (
				installation_id, installation_login, installation_avatar, installation_type,
				installation_created, installation_suspended, installation_updated
			) VALUES (54321, 'testorg', 'avatar.png', 'Organization', 1609459200, 0, 1609459200)
		`)
		if err != nil {
			return err
		}

		// Create a test membership
		_, err = execer.Exec(`
			INSERT INTO memberships (
				membership_installation_id, membership_user_id, membership_role, membership_state,
				membership_created, membership_synced, membership_updated
			) VALUES (54321, 2, 'member', 'active', 1609459200, 1609459200, 1609459200)
		`)
		return err
	})
	require.NoError(t, err)

	tests := []struct {
		name    string
		input   *core.Membership
		wantErr bool
	}{
		{
			name: "update role to admin",
			input: &core.Membership{
				InstallationID: 54321,
				UserID:         2,
				Role:           "admin",
				State:          "active",
				Created:        1609459200,
				Synced:         1609545600,
				Updated:        1609545600,
			},
			wantErr: false,
		},
		{
			name: "update state to pending",
			input: &core.Membership{
				InstallationID: 54321,
				UserID:         2,
				Role:           "admin",
				State:          "pending",
				Created:        1609459200,
				Synced:         1609545600,
				Updated:        1609545600,
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
				got, err := store.Find(ctx, test.input.InstallationID, test.input.UserID)
				require.NoError(t, err)
				assert.Equal(t, test.input, got)
			}
		})
	}
}

func TestUpdateNonExistent(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Try to update a non-existent membership
	membership := &core.Membership{
		InstallationID: 99999,
		UserID:         99999,
		Role:           "member",
		State:          "active",
		Created:        1609459200,
		Synced:         1609459200,
		Updated:        1609459200,
	}

	err = store.Update(ctx, membership)
	// Update should succeed even if no rows are affected (idempotent)
	require.NoError(t, err)
}
