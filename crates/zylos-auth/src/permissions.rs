//! Permission definitions for fine-grained access control.

use serde::{Deserialize, Serialize};

/// Fine-grained permissions that can be checked against roles.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Manage users and roles within the tenant.
    UsersManage,
    /// Create new skills or documents.
    SkillsCreate,
    /// Edit existing skills or documents.
    SkillsEdit,
    /// Delete skills or documents.
    SkillsDelete,
    /// View skills and documents.
    SkillsRead,
    /// Access CRDT collaborative editing sessions.
    CrdtCollaborate,
    /// Manage API tokens and service accounts.
    TokensManage,
    /// View audit logs.
    AuditRead,
    /// Custom permission.
    Custom(String),
}

impl Permission {
    /// Check if a set of roles grants this permission.
    pub fn is_granted_by(&self, roles: &[String]) -> bool {
        for role in roles {
            let granted = match role.as_str() {
                "admin" => true,
                "editor" => matches!(
                    self,
                    Permission::SkillsCreate
                        | Permission::SkillsEdit
                        | Permission::SkillsRead
                        | Permission::CrdtCollaborate
                ),
                "viewer" => matches!(self, Permission::SkillsRead),
                "service_account" => {
                    matches!(self, Permission::SkillsRead | Permission::CrdtCollaborate)
                }
                _ => false,
            };
            if granted {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_permissions() {
        let roles = vec!["admin".to_string()];
        assert!(Permission::UsersManage.is_granted_by(&roles));
        assert!(Permission::SkillsDelete.is_granted_by(&roles));
        assert!(Permission::TokensManage.is_granted_by(&roles));
    }

    #[test]
    fn test_editor_permissions() {
        let roles = vec!["editor".to_string()];
        assert!(Permission::SkillsCreate.is_granted_by(&roles));
        assert!(Permission::SkillsEdit.is_granted_by(&roles));
        assert!(Permission::CrdtCollaborate.is_granted_by(&roles));
        assert!(!Permission::UsersManage.is_granted_by(&roles));
        assert!(!Permission::SkillsDelete.is_granted_by(&roles));
    }

    #[test]
    fn test_viewer_permissions() {
        let roles = vec!["viewer".to_string()];
        assert!(Permission::SkillsRead.is_granted_by(&roles));
        assert!(!Permission::SkillsEdit.is_granted_by(&roles));
    }
}
