package user

import (
	"encoding/json"
	"net/http/httptest"
	"testing"

	"github.com/google/go-cmp/cmp"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
)

func TestFind(t *testing.T) {
	mockUser := &core.User{
		ID:    1,
		Login: "octocat",
	}

	w := httptest.NewRecorder()
	r := httptest.NewRequest("GET", "/api/user", nil)
	r = r.WithContext(
		request.WithUser(r.Context(), mockUser),
	)

	HandleFind()(w, r)
	if got, want := w.Code, 200; want != got {
		t.Errorf("Want response code %d, got %d", want, got)
	}

	// Decode the response structure
	var resp render.Response
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatal(err)
	}

	// Check error field
	if resp.Error {
		t.Errorf("Expected error=false, got error=true")
	}

	// Check reason field
	if resp.Reason != render.ReasonResolved {
		t.Errorf("Expected reason='%s', got '%s'", render.ReasonResolved, resp.Reason)
	}

	// Check data field contains the user
	if resp.Data == nil {
		t.Fatal("Expected data to be present")
	}

	// Convert data back to User struct for comparison
	dataJSON, _ := json.Marshal(resp.Data)
	got := &core.User{}
	_ = json.Unmarshal(dataJSON, got)

	if diff := cmp.Diff(got, mockUser); len(diff) != 0 {
		t.Errorf("User mismatch (-want +got):\n%s", diff)
	}
}
