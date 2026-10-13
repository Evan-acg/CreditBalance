use std::collections::HashMap;

use anyhow::{Result, anyhow};

use crate::domain::balance::ProviderId;

use super::ports::{BalanceProvider, BalanceProviderDescriptor, BalanceProviderRouter};

#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<ProviderId, Box<dyn BalanceProvider>>,
    registration_order: Vec<ProviderId>,
}

impl ProviderRegistry {
    pub fn register(&mut self, provider: Box<dyn BalanceProvider>) -> Result<()> {
        let id = provider.id();
        if self.providers.contains_key(&id) {
            return Err(anyhow!("余额 provider 已重复注册: {id}"));
        }
        self.registration_order.push(id.clone());
        self.providers.insert(id, provider);
        Ok(())
    }

    pub fn descriptors(&self) -> Vec<BalanceProviderDescriptor> {
        self.registration_order
            .iter()
            .filter_map(|id| self.providers.get(id))
            .map(|provider| BalanceProviderDescriptor {
                id: provider.id(),
                display_name: provider.display_name(),
            })
            .collect()
    }
}

impl BalanceProviderRouter for ProviderRegistry {
    fn route(&self, id: &ProviderId) -> Result<&dyn BalanceProvider> {
        self.providers
            .get(id)
            .map(Box::as_ref)
            .ok_or_else(|| anyhow!("不支持的余额平台: {id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubProvider {
        id: &'static str,
        display_name: &'static str,
    }

    impl BalanceProvider for StubProvider {
        fn id(&self) -> ProviderId {
            ProviderId::new(self.id)
        }

        fn display_name(&self) -> &'static str {
            self.display_name
        }

        fn fetch_balance(&self) -> Result<crate::domain::balance::BalanceReport> {
            Ok(crate::domain::balance::BalanceReport {
                is_available: true,
                balance_infos: Vec::new(),
            })
        }
    }

    #[test]
    fn routes_registered_provider_and_exposes_registration_metadata_in_order() {
        // Given two balance providers registered in order
        let mut registry = ProviderRegistry::default();
        registry
            .register(Box::new(StubProvider {
                id: "first",
                display_name: "First",
            }))
            .unwrap();
        registry
            .register(Box::new(StubProvider {
                id: "second",
                display_name: "Second",
            }))
            .unwrap();

        // When the router resolves a provider and the CLI reads provider metadata
        let routed = registry.route(&ProviderId::new("second")).unwrap();
        let descriptors = registry.descriptors();

        // Then routing and registration order are preserved
        assert_eq!(routed.id().as_str(), "second");
        assert_eq!(descriptors[0].id.as_str(), "first");
        assert_eq!(descriptors[1].display_name, "Second");
    }

    #[test]
    fn rejects_duplicate_provider_registration() {
        let mut registry = ProviderRegistry::default();
        let provider = || {
            Box::new(StubProvider {
                id: "same",
                display_name: "Same",
            }) as Box<dyn BalanceProvider>
        };

        registry.register(provider()).unwrap();

        assert!(registry.register(provider()).is_err());
    }
}
