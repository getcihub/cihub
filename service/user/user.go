package user

import (
	"context"

	"github.com/google/go-github/v75/github"
	"github.com/palantir/go-githubapp/githubapp"

	"github.com/getcihub/cihub/core"
)

type service struct {
	client githubapp.ClientCreator
}

func New(client githubapp.ClientCreator) core.UserService {
	return &service{client}
}

func (s *service) Find(ctx context.Context, access, refresh string) (*core.User, error) {
	client, err := s.client.NewTokenClient(access)
	if err != nil {
		return nil, err
	}

	src, _, err := client.Users.Get(ctx, "")
	if err != nil {
		return nil, err
	}
	return convertUser(src), nil
}

func (s *service) ListEmail(ctx context.Context, user *core.User) ([]*core.UserEmail, error) {
	client, err := s.client.NewTokenClient(user.Access)
	if err != nil {
		return nil, err
	}

	opt := &github.ListOptions{PerPage: 100}

	var emails []*github.UserEmail
	for {
		src, res, err := client.Users.ListEmails(ctx, opt)
		if err != nil {
			return nil, err
		}
		emails = append(emails, src...)

		if res.NextPage == 0 {
			break
		}
		opt.Page = res.NextPage
	}

	return convertEmails(emails), nil
}

func convertUser(src *github.User) *core.User {
	dst := &core.User{
		Login:  src.GetLogin(),
		Avatar: src.GetAvatarURL(),
		Email:  src.GetEmail(),
	}
	if !src.GetCreatedAt().IsZero() {
		dst.Created = src.CreatedAt.Unix()
	}
	if !src.GetUpdatedAt().IsZero() {
		dst.Created = src.UpdatedAt.Unix()
	}
	return dst
}

func convertEmails(emails []*github.UserEmail) []*core.UserEmail {
	var out []*core.UserEmail
	for _, email := range emails {
		if !email.GetVerified() {
			continue
		}

		out = append(out, &core.UserEmail{
			Email:    email.GetEmail(),
			Primary:  email.GetPrimary(),
			Verified: email.GetVerified(),
		})
	}

	return out
}
