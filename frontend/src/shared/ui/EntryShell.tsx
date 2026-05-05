import { Card, Flex, Layout, Space, Typography } from "antd";
import { type ReactNode } from "react";

const { Content } = Layout;
const { Paragraph, Text, Title } = Typography;

type EntryShellProps = {
  cardIcon?: ReactNode;
  cardTitle?: ReactNode;
  children: ReactNode;
  description: ReactNode;
  meta?: ReactNode;
  title: ReactNode;
  titleId: string;
};

export function EntryShell({
  cardIcon,
  cardTitle,
  children,
  description,
  meta,
  title,
  titleId,
}: EntryShellProps) {
  return (
    <Layout className="entry-page">
      <Content className="entry-content">
        <section className="entry-shell" aria-labelledby={titleId}>
          <div className="entry-hero">
            <Flex align="center" gap={12} className="entry-brand">
              <div className="entry-brand-mark">LI</div>
              <div className="entry-brand-copy">
                <Text strong>Lab Inventory</Text>
                <div className="entry-brand-subtitle">实验室库存管理</div>
              </div>
            </Flex>

            <div className="entry-copy">
              <Title id={titleId} level={1} className="entry-title">
                {title}
              </Title>
              <Paragraph className="entry-description">{description}</Paragraph>
            </div>

            {meta ? <div className="entry-meta">{meta}</div> : null}
          </div>

          <Card className="entry-card" variant="borderless">
            <Space orientation="vertical" size="large" className="full-width">
              {cardTitle ? (
                <Flex align="center" gap={12} className="entry-card-header">
                  {cardIcon ? <div className="entry-card-icon">{cardIcon}</div> : null}
                  <Title level={2} className="entry-card-title">
                    {cardTitle}
                  </Title>
                </Flex>
              ) : null}

              {children}
            </Space>
          </Card>
        </section>
      </Content>
    </Layout>
  );
}
