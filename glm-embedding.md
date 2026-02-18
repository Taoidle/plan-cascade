> ## Documentation Index
> Fetch the complete documentation index at: https://docs.bigmodel.cn/llms.txt
> Use this file to discover all available pages before exploring further.

# Embedding-3

## <div className="flex items-center"> <svg style={{maskImage: "url(/resource/icon/rectangle-list.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} /> 概览 </div>

Embedding-3 是智谱AI 推出的第三代文本向量化模型，在前代基础上全面升级，提供更强的语义理解能力和更灵活的向量维度选择。该模型支持自定义向量维度，在保持高质量语义表示的同时，为不同应用场景提供了更优的性能和成本平衡。

<CardGroup cols={3}>
  <Card title="价格" icon={<svg style={{maskImage: "url(/resource/icon/coins.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"}/>}>
    0.5 元 / 百万 Tokens
  </Card>

  <Card title="输入模态" icon={<svg style={{maskImage: "url(/resource/icon/arrow-down-right.svg)", WebkitMaskImage: "url(/resource/icon/arrow-down-right.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} />}>
    文本
  </Card>

  <Card title="输出模态" icon={<svg style={{maskImage: "url(/resource/icon/arrow-down-left.svg)", WebkitMaskImage: "url(/resource/icon/arrow-down-left.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} />}>
    向量
  </Card>

  <Card title="上下文窗口" icon={<svg style={{maskImage: "url(/resource/icon/arrow-down-arrow-up.svg)", WebkitMaskImage: "url(/resource/icon/arrow-down-arrow-up.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} />}>
    8K
  </Card>

  <Card title="向量维度" icon={<svg style={{maskImage: "url(/resource/icon/maximize.svg)", WebkitMaskImage: "url(/resource/icon/maximize.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} />}>
    256-2048（可自定义）
  </Card>
</CardGroup>

## <div className="flex items-center"> <svg style={{maskImage: "url(/resource/icon/stars.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} /> 推荐场景 </div>

<AccordionGroup>
  <Accordion title="高精度语义搜索" defaultOpen>
    利用更强的语义理解能力，实现更精准的文档检索和问答系统，特别适合专业领域的知识库构建。
  </Accordion>

  <Accordion title="智能推荐引擎" defaultOpen>
    基于用户行为和内容特征的深度理解，提供更个性化和精准的推荐服务，提升用户体验。
  </Accordion>

  <Accordion title="内容理解与分析" defaultOpen>
    深度分析文本内容的主题、情感和意图，用于舆情监控、内容审核和市场分析。
  </Accordion>

  <Accordion title="知识图谱构建" defaultOpen>
    通过语义向量化技术，自动发现实体关系，构建和完善知识图谱，支持复杂的知识推理。
  </Accordion>
</AccordionGroup>

## <div className="flex items-center"> <svg style={{maskImage: "url(/resource/icon/gauge-high.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} /> 使用资源 </div>

[体验中心](https://bigmodel.cn/trialcenter)：快速测试模型在业务场景上的效果<br />
[接口文档](/api-reference/%E6%A8%A1%E5%9E%8B-api/%E6%96%87%E6%9C%AC%E5%B5%8C%E5%85%A5)：API 调用方式

## <div className="flex items-center"> <svg style={{maskImage: "url(/resource/icon/arrow-up.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} /> 详细介绍 </div>

<Steps>
  <Step title="模型升级" titleSize="h3">
    Embedding-3 在架构和训练数据上都进行了重大升级，显著提升了语义理解的准确性和泛化能力。新模型在多个评测基准上都取得了显著的性能提升。

    **核心升级：**

    * **增强语义理解**：更深层的语义捕捉能力，理解复杂的语言表达
    * **多语言优化**：针对中文、英文等多语言场景进行专门优化
    * **领域适应性**：在科技、金融、医疗等专业领域表现更佳
    * **鲁棒性提升**：对噪声文本和非标准表达有更强的容错能力
  </Step>

  <Step title="灵活维度选择" stepNumber={2} titleSize="h3">
    Embedding-3 支持自定义向量维度，用户可以根据具体应用场景选择最适合的维度，在性能和存储成本之间找到最佳平衡。

    **维度选项：**

    * **2048维（默认）**：最高精度，适合对准确性要求极高的场景
    * **1024维**：高精度与效率的平衡，适合大多数应用场景
    * **512维**：中等精度，适合大规模部署的场景
    * **256维**：较高效率，适合实时性要求高的场景

    **技术参数：**

    * 输入字符串数组中，单条请求最多支持 3072 个 Tokens，且数组最大不得超过 64 条
  </Step>
</Steps>

## <div className="flex items-center"> <svg style={{maskImage: "url(/resource/icon/code.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} /> 调用示例 </div>

以下是一个完整的调用示例，帮助您快速上手 Embedding-3 模型。

<Tabs>
  <Tab title="cURL">
    ```bash  theme={null}
    # 使用默认维度
    curl -X POST \
    https://open.bigmodel.cn/api/paas/v4/embeddings \
    -H "Authorization: Bearer your-api-key" \
    -H "Content-Type: application/json" \
    -d '{
        "model": "embedding-3",
        "input": "这是一段需要向量化的文本"
    }'

    # 自定义维度
    curl -X POST \
    https://open.bigmodel.cn/api/paas/v4/embeddings \
    -H "Authorization: Bearer your-api-key" \
    -H "Content-Type: application/json" \
    -d '{
        "model": "embedding-3",
        "input": "这是一段需要向量化的文本",
        "dimensions": 512
    }'
    ```
  </Tab>

  <Tab title="python">
    **安装 SDK**

    ```bash  theme={null}
    # 安装最新版本
    pip install zai-sdk
    # 或指定版本
    pip install zai-sdk==0.2.2
    ```

    **验证安装**

    ```python  theme={null}
    import zai
    print(zai.__version__)
    ```

    **调用示例**

    ```python  theme={null}
    from zai import ZhipuAiClient

    client = ZhipuAiClient(api_key="your api key")
    response = client.embeddings.create(
        model="embedding-3", #填写需要调用的模型编码
        input=[
            "美食非常美味，服务员也很友好。",
            "这部电影既刺激又令人兴奋。",
            "阅读书籍是扩展知识的好方法。"
        ],
    )
    print(response)
    ```
  </Tab>

  <Tab title="Java">
    **安装 SDK**

    **Maven**

    ```xml  theme={null}
    <dependency>
        <groupId>ai.z.openapi</groupId>
        <artifactId>zai-sdk</artifactId>
        <version>0.3.3</version>
    </dependency>
    ```

    **Gradle (Groovy)**

    ```groovy  theme={null}
    implementation 'ai.z.openapi:zai-sdk:0.3.3'
    ```

    **调用示例**

    ```java  theme={null}
    import ai.z.openapi.ZhipuAiClient;
    import ai.z.openapi.service.embedding.EmbeddingCreateParams;
    import ai.z.openapi.service.embedding.EmbeddingResponse;
    import java.util.Arrays;
    import java.util.List;

    public class Embedding3Example {
        public static void main(String[] args) {
            // 初始化客户端
            ZhipuAiClient client = ZhipuAiClient.builder().ofZHIPU()
                .apiKey("your-api-key")
                .build();

            // 创建向量化请求（自定义维度）
            EmbeddingCreateParams request = EmbeddingCreateParams.builder()
                .model("embedding-3")
                .input(Arrays.asList("Hello world", "How are you?", "How is the weather today?"))
                .dimensions(768)  // 指定768维
                .build();

            // 发送请求
            EmbeddingResponse response = client.embeddings().createEmbeddings(request);
            System.out.println("向量: " + response.getData());
        }
    }
    ```
  </Tab>

  <Tab title="Python(旧)">
    ```python  theme={null}
    from zhipuai import ZhipuAI

    client = ZhipuAI(api_key="your api key")
    response = client.embeddings.create(
        model="embedding-3", #填写需要调用的模型编码
        input=[
            "美食非常美味，服务员也很友好。",
            "这部电影既刺激又令人兴奋。",
            "阅读书籍是扩展知识的好方法。"
        ],
    )
    print(response)
    ```
  </Tab>

  <Tab title="响应示例">
    ```json  theme={null}
    {
        "model": "embedding-3",
        "data": [
    {
        "embedding": [
        -0.02675454691052437,
        0.019060475751757622,
        ......
        -0.005519774276763201,
        0.014949671924114227
        ],
        "index": 0,
        "object": "embedding"
    },
        ...
    {
        "embedding": [
        -0.02675454691052437,
        0.019060475751757622,
        ......
        -0.005519774276763201,
        0.014949671924114227
        ],
        "index": 2,
        "object": "embedding"
    }
        ],
        "object": "list",
        "usage": {
        "completion_tokens": 0,
        "prompt_tokens": 100,
        "total_tokens": 100
    }
    }
    ```
  </Tab>
</Tabs>

## <div className="flex items-center"> <svg style={{maskImage: "url(/resource/icon/feather.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} /> 最佳实践 </div>

<AccordionGroup>
  <Accordion title="维度选择策略">
    根据应用场景选择合适的向量维度：

    * **高精度场景**（如法律文档检索）：使用2048维
    * **通用应用**（如商品推荐）：使用1024或512维
    * **实时应用**（如在线搜索）：使用256维
  </Accordion>

  <Accordion title="性能优化">
    提升向量化性能的建议：

    * 合理使用批处理，单次最多64条文本
    * 预处理文本以去除无关信息
    * 缓存常用文本的向量结果
    * 根据业务需求选择合适的向量维度
  </Accordion>

  <Accordion title="质量提升">
    提高向量质量的技巧：

    * 保持输入文本的完整性和上下文
    * 避免过度分割长文本
    * 统一文本格式和编码
    * 定期评估向量质量并调整策略
  </Accordion>

  <Accordion title="存储优化">
    向量存储的优化建议：

    * 使用适当的向量数据库
    * 建立合适的索引以加速检索
    * 定期清理过期或低质量的向量
    * 考虑向量压缩技术以节省存储空间
  </Accordion>
</AccordionGroup>

## <div className="flex items-center"> <svg style={{maskImage: "url(/resource/icon/square-user.svg)", maskRepeat: "no-repeat", maskPosition: "center center",}} className={"h-6 w-6 bg-primary dark:bg-primary-light !m-0 shrink-0"} /> 用户并发权益 </div>

API 调用会受到速率限制，当前我们限制的维度是请求并发数量（在途请求任务数量）。不同等级的用户并发保障如下。

| V0 | V1  | V2  | V3  |
| :- | :-- | :-- | :-- |
| 50 | 100 | 300 | 500 |
