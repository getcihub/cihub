package installation

import (
	"context"
	"encoding/json"

	"github.com/google/go-github/v72/github"
	"github.com/palantir/go-githubapp/githubapp"
)

type handler struct{}

func New() githubapp.EventHandler {
	return &handler{}
}

func (h *handler) Handles() []string {
	return []string{"installation"}
}

func (h *handler) Handle(ctx context.Context, eventType, deliveryID string, payload []byte) error {
	var event github.InstallationEvent
	if err := json.Unmarshal(payload, &event); err != nil {
		return err
	}

	switch event.GetAction() {
	case "created":
		return h.handleCreated(ctx)
	case "deleted":
		return h.handleDeleted(ctx)
	case "suspend":
		return h.handleSuspend(ctx)
	case "unsuspend":
		return h.handleUnsuspend(ctx)
	default:
		return nil
	}
}

func (h *handler) handleCreated(ctx context.Context) error   { return nil }
func (h *handler) handleDeleted(ctx context.Context) error   { return nil }
func (h *handler) handleSuspend(ctx context.Context) error   { return nil }
func (h *handler) handleUnsuspend(ctx context.Context) error { return nil }
