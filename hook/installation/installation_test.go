package installation

import (
	"context"
	"encoding/json"
	"errors"
	"testing"
	"time"

	"github.com/google/go-github/v72/github"
	"go.uber.org/mock/gomock"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/mock"
)

var noContext = context.TODO()

func TestHandler_Handles(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	events := h.Handles()
	if got, want := len(events), 1; got != want {
		t.Errorf("Want %d handled events, got %d", want, got)
	}
	if got, want := events[0], "installation"; got != want {
		t.Errorf("Want event type %s, got %s", want, got)
	}
}

func TestHandler_Handle_Created_CreateNew(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("created"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
			CreatedAt: &github.Timestamp{Time: time.Now()},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (installation doesn't exist)
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(nil, errors.New("not found"))

	// Expect Create to be called
	mockInstallationStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, installation *core.Installation) error {
			// Verify conversion
			if got, want := installation.ID, int64(123456); got != want {
				t.Errorf("Want installation ID %d, got %d", want, got)
			}
			if got, want := installation.Login, "octocat"; got != want {
				t.Errorf("Want login %s, got %s", want, got)
			}
			if got, want := installation.Type, core.InstallationTypeUser; got != want {
				t.Errorf("Want type %s, got %s", want, got)
			}
			if installation.Created == 0 {
				t.Error("Expected Created timestamp to be set")
			}
			if installation.Updated == 0 {
				t.Error("Expected Updated timestamp to be set")
			}
			return nil
		})

	if err := h.Handle(noContext, "installation", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Created_AlreadyExists(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("created"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingInstallation := &core.Installation{
		ID:    123456,
		Login: "octocat",
	}

	// Expect Find to return existing installation
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(existingInstallation, nil)

	// Create should NOT be called (no expectation set)

	if err := h.Handle(noContext, "installation", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Created_Organization(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("created"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(654321)),
			Account: &github.User{
				Login:     github.Ptr("my-org"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/2?v=4"),
				Type:      github.Ptr("Organization"),
			},
			CreatedAt: &github.Timestamp{Time: time.Now()},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (installation doesn't exist)
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(654321)).
		Return(nil, errors.New("not found"))

	// Expect Create to be called with organization type
	mockInstallationStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, installation *core.Installation) error {
			if got, want := installation.Type, core.InstallationTypeOrganization; got != want {
				t.Errorf("Want type %s, got %s", want, got)
			}
			if got, want := installation.Login, "my-org"; got != want {
				t.Errorf("Want login %s, got %s", want, got)
			}
			return nil
		})

	if err := h.Handle(noContext, "installation", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Deleted_Success(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("deleted"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingInstallation := &core.Installation{
		ID:    123456,
		Login: "octocat",
	}

	// Expect Find to return existing installation
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(existingInstallation, nil)

	// Expect Delete to be called
	mockInstallationStore.EXPECT().
		Delete(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, installation *core.Installation) error {
			if got, want := installation.ID, int64(123456); got != want {
				t.Errorf("Want installation ID %d, got %d", want, got)
			}
			return nil
		})

	if err := h.Handle(noContext, "installation", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Deleted_NotFound(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("deleted"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (installation doesn't exist)
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(nil, errors.New("not found"))

	// Delete should NOT be called (no expectation set)

	// Handler should not fail if installation is not found
	if err := h.Handle(noContext, "installation", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Suspend_Success(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("suspend"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingInstallation := &core.Installation{
		ID:        123456,
		Login:     "octocat",
		Suspended: 0,
		Updated:   time.Now().Unix(),
	}

	// Expect Find to return existing installation
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(existingInstallation, nil)

	// Expect Update to be called
	mockInstallationStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, installation *core.Installation) error {
			if installation.Suspended == 0 {
				t.Error("Expected Suspended timestamp to be set")
			}
			if installation.Updated == 0 {
				t.Error("Expected Updated timestamp to be set")
			}
			return nil
		})

	if err := h.Handle(noContext, "installation", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_Unsuspend_Success(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("unsuspend"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	suspendedTime := time.Now().Add(-1 * time.Hour).Unix()
	existingInstallation := &core.Installation{
		ID:        123456,
		Login:     "octocat",
		Suspended: suspendedTime,
	}

	// Expect Find to return existing installation
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(existingInstallation, nil)

	// Expect Update to be called
	mockInstallationStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		DoAndReturn(func(ctx context.Context, installation *core.Installation) error {
			if got, want := installation.Suspended, int64(0); got != want {
				t.Errorf("Want Suspended = 0, got %d", got)
			}
			if installation.Updated == 0 {
				t.Error("Expected Updated timestamp to be set")
			}
			return nil
		})

	if err := h.Handle(noContext, "installation", "test-delivery", payload); err != nil {
		t.Errorf("Handle failed: %v", err)
	}
}

func TestHandler_Handle_InvalidPayload(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	invalidPayload := []byte(`{invalid json}`)

	err := h.Handle(noContext, "installation", "test-delivery", invalidPayload)
	if err == nil {
		t.Error("Expected error for invalid JSON payload")
	}
}

func TestHandler_Handle_MissingInstallation(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action:       github.Ptr("created"),
		Installation: nil, // Missing
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	err = h.Handle(noContext, "installation", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error for missing installation")
	}
	if got := err.Error(); got != "hook: missing installation in event payload" {
		t.Errorf("Want error message 'hook: missing installation in event payload', got %q", got)
	}
}

func TestHandler_Handle_CreateError(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("created"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// Expect Find to return error (installation doesn't exist)
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(nil, errors.New("not found"))

	// Expect Create to fail
	mockInstallationStore.EXPECT().
		Create(gomock.Any(), gomock.Any()).
		Return(errors.New("database error"))

	err = h.Handle(noContext, "installation", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error when Create fails")
	}
}

func TestHandler_Handle_DeleteError(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("deleted"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingInstallation := &core.Installation{
		ID:    123456,
		Login: "octocat",
	}

	// Expect Find to return existing installation
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(existingInstallation, nil)

	// Expect Delete to fail
	mockInstallationStore.EXPECT().
		Delete(gomock.Any(), gomock.Any()).
		Return(errors.New("database error"))

	err = h.Handle(noContext, "installation", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error when Delete fails")
	}
}

func TestHandler_Handle_SuspendError(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("suspend"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	existingInstallation := &core.Installation{
		ID:        123456,
		Login:     "octocat",
		Suspended: 0,
	}

	// Expect Find to return existing installation
	mockInstallationStore.EXPECT().
		Find(gomock.Any(), int64(123456)).
		Return(existingInstallation, nil)

	// Expect Update to fail
	mockInstallationStore.EXPECT().
		Update(gomock.Any(), gomock.Any()).
		Return(errors.New("database error"))

	err = h.Handle(noContext, "installation", "test-delivery", payload)
	if err == nil {
		t.Error("Expected error when Update fails")
	}
}

func TestHandler_Handle_UnknownAction(t *testing.T) {
	ctrl := gomock.NewController(t)
	defer ctrl.Finish()

	mockInstallationStore := mock.NewMockInstallationStore(ctrl)
	h := New(mockInstallationStore)

	event := &github.InstallationEvent{
		Action: github.Ptr("unknown_action"),
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
		},
	}

	payload, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}

	// No store calls expected for unknown action
	// Handler should not fail, just log a warning
	if err := h.Handle(noContext, "installation", "test-delivery", payload); err != nil {
		t.Errorf("Handle should not fail for unknown action, got: %v", err)
	}
}

func TestConvertInstallationEventToInstallation(t *testing.T) {
	createdAt := time.Unix(1234567890, 0)

	event := github.InstallationEvent{
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
			CreatedAt: &github.Timestamp{Time: createdAt},
		},
	}

	installation := convertInstallationEventToInstallation(event)

	tests := []struct {
		name string
		got  interface{}
		want interface{}
	}{
		{"ID", installation.ID, int64(123456)},
		{"Login", installation.Login, "octocat"},
		{"Avatar", installation.Avatar, "https://avatars.githubusercontent.com/u/1?v=4"},
		{"Type", installation.Type, core.InstallationTypeUser},
		{"Created", installation.Created, createdAt.Unix()},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.got != tt.want {
				t.Errorf("Want %s = %v, got %v", tt.name, tt.want, tt.got)
			}
		})
	}
}

func TestConvertInstallationEventToInstallation_Organization(t *testing.T) {
	event := github.InstallationEvent{
		Installation: &github.Installation{
			ID: github.Ptr(int64(654321)),
			Account: &github.User{
				Login:     github.Ptr("my-org"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/2?v=4"),
				Type:      github.Ptr("Organization"),
			},
		},
	}

	installation := convertInstallationEventToInstallation(event)

	if got, want := installation.Type, core.InstallationTypeOrganization; got != want {
		t.Errorf("Want type %s, got %s", want, got)
	}
	if got, want := installation.Login, "my-org"; got != want {
		t.Errorf("Want login %s, got %s", want, got)
	}
}

func TestConvertInstallationEventToInstallation_NoTimestamp(t *testing.T) {
	event := github.InstallationEvent{
		Installation: &github.Installation{
			ID: github.Ptr(int64(123456)),
			Account: &github.User{
				Login:     github.Ptr("octocat"),
				AvatarURL: github.Ptr("https://avatars.githubusercontent.com/u/1?v=4"),
				Type:      github.Ptr("User"),
			},
			// No CreatedAt set
		},
	}

	installation := convertInstallationEventToInstallation(event)

	// Created should be zero when not set in event
	if installation.Created != 0 {
		t.Errorf("Want Created = 0 for missing CreatedAt, got %d", installation.Created)
	}
}

func TestConvertAccountType(t *testing.T) {
	tests := []struct {
		name string
		got  string
		want string
	}{
		{"Organization", convertAccountType("Organization"), core.InstallationTypeOrganization},
		{"User", convertAccountType("User"), core.InstallationTypeUser},
		{"Unknown", convertAccountType("Unknown"), core.InstallationTypeUser},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.got != tt.want {
				t.Errorf("Want %s, got %s", tt.want, tt.got)
			}
		})
	}
}
