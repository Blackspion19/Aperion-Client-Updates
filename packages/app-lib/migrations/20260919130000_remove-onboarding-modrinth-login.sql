-- The Modrinth account login has been removed from this launcher, so the
-- onboarding checklist no longer has a "logged into Modrinth" step.
ALTER TABLE onboarding_checklist DROP COLUMN has_logged_into_modrinth;
