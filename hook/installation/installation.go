package installation

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/go-github/v72/github"
	"github.com/palantir/go-githubapp/githubapp"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

type handler struct {
	installations core.InstallationStore
}

func New(installations core.InstallationStore) githubapp.EventHandler {
	return &handler{installations}
}

func (h *handler) Handles() []string {
	return []string{"installation"}
}

func (h *handler) Handle(ctx context.Context, eventType, deliveryID string, payload []byte) error {
	var event github.InstallationEvent
	if err := json.Unmarshal(payload, &event); err != nil {
		return fmt.Errorf("hook: failed to parse installation event payload: %w", err)
	}

	// Sanity check
	if event.Installation == nil {
		return fmt.Errorf("hook: missing installation in event payload")
	}

	log := logger.
		FromContext(ctx).
		WithFields(
			logrus.Fields{
				"action":             event.GetAction(),
				"delivery_id":        deliveryID,
				"event":              eventType,
				"installation_id":    event.Installation.GetID(),
				"installation_login": event.Installation.Account.GetLogin(),
			},
		)
	log.Infoln("hook: received installation event")

	ctx = logger.WithContext(ctx, log)
	switch event.GetAction() {
	case "created":
		return h.handleCreated(ctx, event)
	case "deleted":
		return h.handleDeleted(ctx, event)
	case "suspend":
		return h.handleSuspend(ctx, event)
	case "unsuspend":
		return h.handleUnsuspend(ctx, event)
	default:
	}

	log.Warnf("hook: unknown installation action: %s", event.GetAction())

	return nil
}

func (h *handler) handleCreated(ctx context.Context, event github.InstallationEvent) error {
	log := logger.FromContext(ctx)

	installation := convertInstallationEventToInstallation(event)
	now := time.Now().Unix()
	installation.Created = now
	installation.Updated = now

	// Check if installation already exists (idempotency)
	_, err := h.installations.Find(ctx, installation.ID)
	if err == nil {
		log.Debugln("hook: installation already exists, skip")
		return nil
	}

	// Create new installation
	err = h.installations.Create(ctx, installation)
	if err != nil {
		log.WithError(err).
			Warnln("hook: installation creation failed")
		return fmt.Errorf("hook: failed to create installation, err: %w", err)
	}

	log.Infoln("hook: installation created")
	return nil
}

func (h *handler) handleDeleted(ctx context.Context, event github.InstallationEvent) error {
	log := logger.FromContext(ctx)

	installation, err := h.installations.Find(ctx, event.Installation.GetID())
	if err != nil {
		log.WithError(err).
			Debugln("hook: cannot find installation")
		return nil
	}

	err = h.installations.Delete(ctx, installation)
	if err != nil {
		log.WithError(err).
			Debugln("hook: cannot delete installation")
		return fmt.Errorf("hook: failed to delete installation, err: %w", err)
	}

	log.Infoln("hook: installation deleted")
	return nil
}

func (h *handler) handleSuspend(ctx context.Context, event github.InstallationEvent) error {
	log := logger.FromContext(ctx)
	installation, err := h.installations.Find(ctx, event.Installation.GetID())
	if err != nil {
		log.WithError(err).
			Debugln("hook: cannot find installation")
		return fmt.Errorf("hook: failed to find installation, err: %w", err)
	}

	installation.Suspended = time.Now().Unix()
	installation.Updated = time.Now().Unix()

	err = h.installations.Update(ctx, installation)
	if err != nil {
		log.WithError(err).
			Debugln("hook: cannot update installation")
		return fmt.Errorf("hook: failed to update installation, err: %w", err)
	}

	log.Infoln("hook: suspended installation")
	return nil
}

func (h *handler) handleUnsuspend(ctx context.Context, event github.InstallationEvent) error {
	log := logger.FromContext(ctx)
	installation, err := h.installations.Find(ctx, event.Installation.GetID())
	if err != nil {
		return fmt.Errorf("hook: failed to find installation, err: %w", err)
	}

	installation.Suspended = 0
	installation.Updated = time.Now().Unix()

	if err := h.installations.Update(ctx, installation); err != nil {
		return fmt.Errorf("hook: failed to update installation, err: %w", err)
	}

	log.Infoln("hook: unsuspended installation")
	return nil
}

// convertInstallationEventToInstallation converts a GitHub InstallationEvent
// to a core.Installation model.
func convertInstallationEventToInstallation(event github.InstallationEvent) *core.Installation {
	inst := event.Installation
	account := inst.Account

	installation := &core.Installation{
		ID:     inst.GetID(),
		Login:  account.GetLogin(),
		Avatar: account.GetAvatarURL(),
		Type:   convertAccountType(account.GetType()),
	}

	// Convert GitHub timestamps to Unix timestamps if available
	if createdAt := inst.GetCreatedAt(); !createdAt.IsZero() {
		installation.Created = createdAt.Unix()
	}

	return installation
}

// convertAccountType converts GitHub account type string to core installation type.
func convertAccountType(s string) string {
	switch s {
	case "Organization":
		return core.InstallationTypeOrganization
	default:
		return core.InstallationTypeUser
	}
}
