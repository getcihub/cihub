package syncer

import (
	"context"
	"runtime/debug"
	"time"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
)

type syncer struct {
	batch         core.Batcher
	installationz core.InstallationService
	installations core.InstallationStore
	users         core.UserStore
}

// New returns a new membership synchronizer
func New(
	batch core.Batcher,
	installationz core.InstallationService,
	installations core.InstallationStore,
	users core.UserStore,
) core.Syncer {
	return &syncer{
		batch:         batch,
		installationz: installationz,
		installations: installations,
		users:         users,
	}
}

func (s *syncer) Sync(ctx context.Context, user *core.User) (*core.Batch, error) {
	logger := logrus.WithField("login", user.Login)
	logger.Debugln("syncer: begin membership sync")

	defer func() {
		// taking the paranoid approach to recover from
		// a panic that should absolutely never happen.
		if err := recover(); err != nil {
			logger = logger.WithField("error", err)
			logger.Errorf("syncer: unexpected panic\n%s\n", debug.Stack())
		}

		// when the synchronization process is complete
		// be sure to update the user sync date.
		user.Syncing = false
		user.Synced = time.Now().Unix()
		s.users.Update(context.Background(), user)
	}()

	if !user.Syncing {
		user.Syncing = true
		err := s.users.Update(ctx, user)
		if err != nil {
			logger = logger.WithError(err)
			logger.Warnln("syncer: cannot update user")
			return nil, err
		}
	}

	batch := &core.Batch{}
	remote := map[int64]*core.Installation{}
	local := map[int64]*core.Installation{}

	// Step 1.
	// Get the list of installations the user as access
	// to from GitHub along wih organization memberships.

	{
		installations, err := s.installationz.List(ctx, user)
		if err != nil {
			logger = logger.WithError(err)
			logger.Warnln("syncer: cannot get remote installation list")
			return nil, err
		}

		for _, installation := range installations {
			// For each installation, if the account associated
			// is an organization, fetch the membership status for
			// the user and that organization
			if installation.Type == core.InstallationTypeOrganization {
				membership, err := s.installationz.FindMembership(ctx, user, installation.Login)
				if err != nil {
					logger = logger.WithError(err)
					logger.Warnln("syncer: cannot get org membership")
					return nil, err
				}
				installation.Membership = membership
			}

			// Handle the case an installation is associated with
			// a user account and it matches the authenticated user.
			if installation.Login == user.Login {
				installation.Membership = &core.Membership{
					State: "active",
					Role:  core.MembershipRoleOwner,
				}
			}

			// Add installation to remote cache
			remote[installation.ID] = installation
		}
	}

	// Step 2.
	// Get the list of installations stored on the local database.

	{
		installations, err := s.installations.List(ctx, user)
		if err != nil {
			logger = logger.WithError(err)
			logger.Warnln("syncer: cannot get cached membership list")
			return nil, err
		}

		for _, installation := range installations {
			local[installation.ID] = installation
		}
	}

	// Step 3.
	// Find installations that exist on GitHub, but do
	// not exist locally. Insert.
	for k, v := range remote {
		_, ok := local[k]
		if !ok {
			batch.Insert = append(batch.Insert, v)
		}
	}

	// Step 4.
	// Find installations that exist on GitHub and on the
	// local database but with incorrect data. Update.
	for k, v := range remote {
		vv, ok := local[k]
		if !ok {
			continue
		}

		// Check membership differences for all installation types
		if diff(v, vv) {
			batch.Update = append(batch.Update, v)
		}
	}

	// Step 5.
	// Find installations that exist in the local database,
	// but not in GitHub. Revoke.
	//
	for k, v := range local {
		_, ok := remote[k]
		if !ok {
			batch.Revoke = append(batch.Revoke, v)
		}
	}

	// Step 6.
	// Update database.

	if err := s.batch.Batch(ctx, user, batch); err != nil {
		logger = logger.WithError(err)
		logger.Warnln("syncer: cannot batch update")
		return nil, err
	}

	logger.Debugln("syncer: finished installation sync")
	return batch, nil
}
