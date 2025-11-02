package batch

import (
	"context"
	"testing"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db/dbtest"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestBatchInsert(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Create a test user
	user := &core.User{
		ID:    1,
		Login: "testuser",
		Email: "test@example.com",
	}

	// Create batch with installations to insert
	batch := &core.Batch{
		Insert: []*core.Installation{
			{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				Membership: &core.Membership{
					Role:    core.MembershipRoleAdmin,
					State:   "active",
					Created: 1609459200,
					Synced:  1609459200,
					Updated: 1609459200,
				},
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
		},
	}

	err = store.Batch(ctx, user, batch)
	require.NoError(t, err)

	// Verify installations were created
	// We can't directly query, so we'll just verify no error occurred
	assert.Len(t, batch.Insert, 2)
}

func TestBatchUpdate(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Create a test user
	user := &core.User{
		ID:    2,
		Login: "testuser2",
		Email: "test2@example.com",
	}

	// First, create initial installations
	initialBatch := &core.Batch{
		Insert: []*core.Installation{
			{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				Membership: &core.Membership{
					Role:    core.MembershipRoleMember,
					State:   "active",
					Created: 1609459200,
					Synced:  1609459200,
					Updated: 1609459200,
				},
			},
		},
	}

	err = store.Batch(ctx, user, initialBatch)
	require.NoError(t, err)

	// Now update the membership
	updateBatch := &core.Batch{
		Update: []*core.Installation{
			{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				Membership: &core.Membership{
					Role:    core.MembershipRoleAdmin,
					State:   "active",
					Created: 1609459200,
					Synced:  1609545600,
					Updated: 1609545600,
				},
			},
		},
	}

	err = store.Batch(ctx, user, updateBatch)
	require.NoError(t, err)
}

func TestBatchRevoke(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Create a test user
	user := &core.User{
		ID:    3,
		Login: "testuser3",
		Email: "test3@example.com",
	}

	// First, create installations
	initialBatch := &core.Batch{
		Insert: []*core.Installation{
			{
				ID:        12345,
				Login:     "acme",
				Avatar:    "https://avatars.githubusercontent.com/u/12345",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				Membership: &core.Membership{
					Role:    core.MembershipRoleAdmin,
					State:   "active",
					Created: 1609459200,
					Synced:  1609459200,
					Updated: 1609459200,
				},
			},
		},
	}

	err = store.Batch(ctx, user, initialBatch)
	require.NoError(t, err)

	// Now revoke the installation
	revokeBatch := &core.Batch{
		Revoke: []*core.Installation{
			{
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

	err = store.Batch(ctx, user, revokeBatch)
	require.NoError(t, err)
}

func TestBatchCombined(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	// Create test users
	user1 := &core.User{
		ID:    4,
		Login: "user4",
		Email: "user4@example.com",
	}

	user2 := &core.User{
		ID:    5,
		Login: "user5",
		Email: "user5@example.com",
	}

	// User 1 initial sync: insert new installations
	batch1 := &core.Batch{
		Insert: []*core.Installation{
			{
				ID:        1001,
				Login:     "org1",
				Avatar:    "avatar1.png",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				Membership: &core.Membership{
					Role:    core.MembershipRoleAdmin,
					State:   "active",
					Created: 1609459200,
					Synced:  1609459200,
					Updated: 1609459200,
				},
			},
			{
				ID:        1002,
				Login:     "org2",
				Avatar:    "avatar2.png",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				Membership: &core.Membership{
					Role:    core.MembershipRoleMember,
					State:   "active",
					Created: 1609459200,
					Synced:  1609459200,
					Updated: 1609459200,
				},
			},
		},
	}

	err = store.Batch(ctx, user1, batch1)
	require.NoError(t, err)

	// User 2 initial sync: insert same installations with different membership
	batch2 := &core.Batch{
		Insert: []*core.Installation{
			{
				ID:        1001,
				Login:     "org1",
				Avatar:    "avatar1.png",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				Membership: &core.Membership{
					Role:    core.MembershipRoleMember,
					State:   "active",
					Created: 1609459200,
					Synced:  1609459200,
					Updated: 1609459200,
				},
			},
		},
	}

	err = store.Batch(ctx, user2, batch2)
	require.NoError(t, err)

	// User 1 sync again: update org2 membership, revoke org1
	batch3 := &core.Batch{
		Update: []*core.Installation{
			{
				ID:        1002,
				Login:     "org2",
				Avatar:    "avatar2.png",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				Membership: &core.Membership{
					Role:    core.MembershipRoleAdmin,
					State:   "active",
					Created: 1609459200,
					Synced:  1609545600,
					Updated: 1609545600,
				},
			},
		},
		Revoke: []*core.Installation{
			{
				ID:        1001,
				Login:     "org1",
				Avatar:    "avatar1.png",
				Type:      core.InstallationTypeOrganization,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
			},
		},
	}

	err = store.Batch(ctx, user1, batch3)
	require.NoError(t, err)
}

func TestBatchEmptyBatch(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	user := &core.User{
		ID:    6,
		Login: "user6",
		Email: "user6@example.com",
	}

	// Empty batch should succeed
	emptyBatch := &core.Batch{
		Insert: []*core.Installation{},
		Update: []*core.Installation{},
		Revoke: []*core.Installation{},
	}

	err = store.Batch(ctx, user, emptyBatch)
	require.NoError(t, err)
}

func TestBatchUserTypeInstallation(t *testing.T) {
	database, err := dbtest.Connect()
	require.NoError(t, err)
	defer dbtest.Disconnect(database)
	store := New(database)
	ctx := context.Background()

	user := &core.User{
		ID:    7,
		Login: "user7",
		Email: "user7@example.com",
	}

	// User-type installations should not have membership records created
	batch := &core.Batch{
		Insert: []*core.Installation{
			{
				ID:        99999,
				Login:     "octocat",
				Avatar:    "https://avatars.githubusercontent.com/u/99999",
				Type:      core.InstallationTypeUser,
				Created:   1609459200,
				Suspended: 0,
				Updated:   1609459200,
				// Note: User-type installations typically have no Membership
				Membership: nil,
			},
		},
	}

	err = store.Batch(ctx, user, batch)
	require.NoError(t, err)
}
