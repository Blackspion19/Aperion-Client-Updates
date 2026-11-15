use crate::State;
use crate::event::emit::emit_onboarding_checklist;
use crate::state::{
    OnboardingChecklist, OnboardingChecklistItem, get_onboarding_checklist,
    mark_onboarding_checklist_item,
};

#[tracing::instrument]
pub async fn get() -> crate::Result<OnboardingChecklist> {
    let state = State::get().await?;

    // Self-heal: if an account or an instance already exists, the matching
    // step must be marked as done even if the original event was missed.
    if !crate::state::Credentials::get_all(&state.pool)
        .await?
        .is_empty()
    {
        let _ = mark(OnboardingChecklistItem::LoggedIntoMinecraft).await;
    }
    if !crate::api::instance::list().await?.is_empty() {
        let _ = mark(OnboardingChecklistItem::CreatedInstance).await;
    }

    get_onboarding_checklist(&state.pool).await
}

pub(crate) async fn mark_created_instance() -> crate::Result<()> {
    mark(OnboardingChecklistItem::CreatedInstance).await
}

pub(crate) async fn mark_logged_into_minecraft() -> crate::Result<()> {
    mark(OnboardingChecklistItem::LoggedIntoMinecraft).await
}

async fn mark(item: OnboardingChecklistItem) -> crate::Result<()> {
    let state = State::get().await?;
    if let Some(checklist) =
        mark_onboarding_checklist_item(item, &state.pool).await?
    {
        emit_onboarding_checklist(checklist).await?;
    }

    Ok(())
}
