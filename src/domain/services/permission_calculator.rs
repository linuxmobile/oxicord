use crate::domain::entities::{Channel, Member, Permissions, Role};

pub struct PermissionCalculator;

impl PermissionCalculator {
    #[must_use]
    pub fn compute_permissions(
        guild_id: u64,
        channel: &Channel,
        member: &Member,
        guild_roles: &[Role],
    ) -> Permissions {
        let mut permissions = Permissions::empty();

        if let Some(everyone_role) = guild_roles.iter().find(|r| r.id.as_u64() == guild_id) {
            permissions = everyone_role.permissions;
        }

        for role_id in &member.roles {
            if let Some(role) = guild_roles.iter().find(|r| r.id == *role_id) {
                permissions |= role.permissions;
            }
        }

        if permissions.contains(Permissions::ADMINISTRATOR) {
            return Permissions::all();
        }

        if let Some(overwrite) = channel
            .permission_overwrites()
            .iter()
            .find(|o| o.id == guild_id.to_string())
            && let (Ok(allow), Ok(deny)) = (
                overwrite.allow.parse::<u64>(),
                overwrite.deny.parse::<u64>(),
            )
        {
            permissions &= !Permissions::from_bits_truncate(deny);
            permissions |= Permissions::from_bits_truncate(allow);
        }

        let mut role_allow = Permissions::empty();
        let mut role_deny = Permissions::empty();

        for role_id in &member.roles {
            if let Some(overwrite) = channel
                .permission_overwrites()
                .iter()
                .find(|o| o.id == role_id.to_string())
                && let (Ok(allow), Ok(deny)) = (
                    overwrite.allow.parse::<u64>(),
                    overwrite.deny.parse::<u64>(),
                )
            {
                role_allow |= Permissions::from_bits_truncate(allow);
                role_deny |= Permissions::from_bits_truncate(deny);
            }
        }

        permissions &= !role_deny;
        permissions |= role_allow;

        if let Some(user) = &member.user
            && let Some(overwrite) = channel
                .permission_overwrites()
                .iter()
                .find(|o| o.id == user.id().to_string())
            && let (Ok(allow), Ok(deny)) = (
                overwrite.allow.parse::<u64>(),
                overwrite.deny.parse::<u64>(),
            )
        {
            permissions &= !Permissions::from_bits_truncate(deny);
            permissions |= Permissions::from_bits_truncate(allow);
        }

        permissions
    }
}
