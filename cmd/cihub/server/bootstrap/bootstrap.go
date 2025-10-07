package bootstrap

import (
	"context"
	"time"

	"github.com/dchest/uniuri"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
	"github.com/sirupsen/logrus"
)

// Bootstrapper bootstraps the system with the initial account.
type Bootstrapper struct {
	users core.UserStore
}

// New returns a new account bootstrapper.
func New(users core.UserStore) *Bootstrapper {
	return &Bootstrapper{users}
}

// Bootstrap creates a user account. If the account already exists,
// no account is created and a nil error is returned.
func (b *Bootstrapper) Bootstrap(ctx context.Context, user *core.User) error {
	if user.Login == "" {
		return nil
	}

	log := logrus.WithFields(
		logrus.Fields{
			"login": user.Login,
			"admin": user.Admin,
			"token": user.Token,
		},
	)

	log.Debugln("bootstrap: create account")

	existing, err := b.users.FindLogin(ctx, user.Login)
	if err == nil && existing != nil {
		ctx = logger.WithContext(ctx, log)
		return b.update(ctx, user, existing)
	}

	user.Active = true
	user.Created = time.Now().Unix()
	user.Updated = time.Now().Unix()
	if user.Token == "" {
		user.Token = uniuri.NewLen(32)
	}

	err = b.users.Create(ctx, user)
	if err != nil {
		log = log.WithError(err)
		log.Errorln("bootstrap: cannot create account")
		return err
	}

	log = log.WithField("token", user.Token)
	log.Infoln("bootstrap: account created")
	return nil
}

func (b *Bootstrapper) update(ctx context.Context, src, dst *core.User) error {
	log := logger.FromContext(ctx)
	log.Debugln("bootstrap: updating account")

	var updated bool
	if src.Token != dst.Token && src.Token != "" {
		log.Infoln("bootstrap: found updated user token")
		dst.Token = src.Token
		updated = true
	}

	if src.Admin != dst.Admin {
		log.Infoln("bootstrap: found updated admin flag")
		dst.Admin = src.Admin
		updated = true
	}

	if !updated {
		log.Debugln("bootstrap: account already up-to-date")
		return nil
	}

	dst.Updated = time.Now().Unix()
	err := b.users.Update(ctx, dst)
	if err != nil {
		log = log.WithError(err)
		log.Errorln("bootstrap: cannot update account")
		return err
	}

	log.Infoln("bootstrap: account successfully updated")
	return nil
}
