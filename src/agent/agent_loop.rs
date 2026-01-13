use crate::cli::Console;
use crate::conversation::Conversation;
use crate::llm::AnthropicProvider;
use anyhow::Result;

/// Main agent that orchestrates the conversation loop
pub struct Agent {
    console: Console,
    llm_provider: AnthropicProvider,
    conversation: Conversation,
    system_prompt: Option<String>,
}

impl Agent {
    /// Create a new Agent with a console and LLM provider
    /// Automatically creates a new conversation
    pub fn new(console: Console, llm_provider: AnthropicProvider) -> Result<Self> {
        tracing::info!("Creating new Agent");

        // Create a new conversation
        let conversation = Conversation::new()?;
        tracing::info!("Conversation initialized: {}", conversation.id());

        Ok(Self {
            console,
            llm_provider,
            conversation,
            system_prompt: None,
        })
    }

    /// Set a system prompt for the agent
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Result<Self> {
        self.system_prompt = Some(prompt.into());
        Ok(self)
    }

    /// Get the conversation ID
    pub fn conversation_id(&self) -> &str {
        self.conversation.id()
    }

    /// Get a reference to the console
    pub fn console(&self) -> &Console {
        &self.console
    }

    /// Run the main agent loop
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Starting agent loop");
        self.console.print_banner();

        loop {
            // Read user input
            let user_input = match self.console.read_input() {
                Ok(input) => {
                    tracing::debug!("User input received: {}", input);
                    input
                }
                Err(e) => {
                    tracing::error!("Failed to read user input: {}", e);
                    self.console.print_error(&format!("Failed to read input: {}", e));
                    continue;
                }
            };

            // Check for exit commands
            if user_input.to_lowercase() == "exit" || user_input.to_lowercase() == "quit" {
                tracing::info!("User requested exit");
                self.console.print_system("Goodbye!");
                break;
            }

            // Skip empty input
            if user_input.trim().is_empty() {
                tracing::debug!("Empty input, skipping");
                continue;
            }

            // Print separator for readability
            self.console.println();

            // Process the message
            tracing::info!("Processing user message");
            if let Err(e) = self.process_message(&user_input).await {
                tracing::error!("Error processing message: {:?}", e);
                self.console.print_error(&format!("Error processing message: {}", e));
            }

            // Print separator after response
            self.console.println();
            self.console.print_separator();
        }

        tracing::info!("Agent loop ended");
        Ok(())
    }

    /// Process a single user message
    async fn process_message(&mut self, user_message: &str) -> Result<()> {
        tracing::debug!("Processing message: {}", user_message);

        // Get conversation history before adding new message
        let history = self.conversation.get_messages()?;
        tracing::debug!("Retrieved {} previous messages from conversation history", history.len());

        // Get the complete response with conversation context
        let response = self
            .llm_provider
            .send_message(user_message, &history, self.system_prompt.as_deref())
            .await
            .map_err(|e| {
                tracing::error!("Failed to get LLM response: {:?}", e);
                e
            })?;

        tracing::debug!("Response received, length: {} chars", response.len());

        // Save user message to conversation history
        self.conversation.add_user_message(user_message)?;
        tracing::debug!("User message saved to conversation history");

        // Save assistant response to conversation history
        self.conversation.add_assistant_message(&response)?;
        tracing::debug!("Assistant message saved to conversation history");

        // Print the response
        self.console.print_assistant(&response);

        tracing::info!("Message processing complete");

        Ok(())
    }
}
